//! Uniform world geometry in a CSS-sized viewport, with logical canvas input.
use crate::visual::{Point, Rect};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub scale: f64,
    pub x: f64,
    pub y: f64,
}
impl Camera {
    pub fn zoom(&mut self, factor: f64, anchor: Point) {
        let previous = self.scale;
        self.scale = (self.scale * factor).clamp(0.000001, 4.0);
        self.x = anchor.0 - (anchor.0 - self.x) * self.scale / previous;
        self.y = anchor.1 - (anchor.1 - self.y) * self.scale / previous;
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub width: f64,
    pub height: f64,
}
impl Viewport {
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width: width.max(1.0),
            height: height.max(1.0),
        }
    }
    pub fn aspect_y(self) -> f64 {
        0.6 * self.width / self.height
    }
    pub fn screen(self, p: Point, c: Camera) -> Point {
        (p.0 * c.scale + c.x, p.1 * c.scale * self.aspect_y() + c.y)
    }
    pub fn world(self, p: Point, c: Camera) -> Point {
        (
            (p.0 - c.x) / c.scale,
            (p.1 - c.y) / (c.scale * self.aspect_y()),
        )
    }
    pub fn css(self, p: Point) -> Point {
        (p.0 * self.width / 1000.0, p.1 * self.height / 600.0)
    }
    pub fn logical(self, p: Point) -> Point {
        (p.0 * 1000.0 / self.width, p.1 * 600.0 / self.height)
    }
    pub fn distance(self, a: Point, b: Point) -> f64 {
        let (a, b) = (self.css(a), self.css(b));
        (a.0 - b.0).hypot(a.1 - b.1)
    }
    pub fn centered(self, p: Point, scale: f64, css_center: Point) -> Camera {
        let center = self.logical(css_center);
        Camera {
            scale,
            x: center.0 - p.0 * scale,
            y: center.1 - p.1 * scale * self.aspect_y(),
        }
    }
    pub fn frame(self, points: &[Point], area: Rect) -> Option<Camera> {
        let first = *points.first()?;
        let (mut lo, mut hi) = (first, first);
        for &p in points {
            lo.0 = lo.0.min(p.0);
            lo.1 = lo.1.min(p.1);
            hi.0 = hi.0.max(p.0);
            hi.1 = hi.1.max(p.1);
        }
        let css_scale = ((area.right - area.left).max(1.0) / (hi.0 - lo.0 + 100.0))
            .min((area.bottom - area.top).max(1.0) / (hi.1 - lo.1 + 100.0));
        let scale = (css_scale * 1000.0 / self.width).clamp(0.000001, 1.6);
        Some(self.centered(
            ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0),
            scale,
            (
                (area.left + area.right) / 2.0,
                (area.top + area.bottom) / 2.0,
            ),
        ))
    }
}
