//! Geometry helpers: DPI scaling, pixel snapping, scroll regions and the
//! draggable-widget placement algorithm.

use epaint::{Pos2, Rect, Vec2};

/// Scale helper: converts design pixels (the Roblox offsets in the original
/// library) into screen points for the current DPI scale.
#[derive(Clone, Copy, Debug)]
pub struct M {
    pub s: f32,
    pub ppp: f32,
}

impl M {
    pub fn px(&self, v: f32) -> f32 {
        v * self.s
    }

    pub fn v(&self, x: f32, y: f32) -> Vec2 {
        Vec2::new(x * self.s, y * self.s)
    }

    /// Snap a coordinate to the physical pixel grid (crisp 1px lines).
    pub fn snap(&self, v: f32) -> f32 {
        (v * self.ppp).round() / self.ppp
    }

    pub fn snap_pos(&self, p: Pos2) -> Pos2 {
        Pos2::new(self.snap(p.x), self.snap(p.y))
    }

    pub fn snap_rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.snap_pos(r.min), self.snap_pos(r.max))
    }

    /// Width of a hairline in points (at least one physical pixel).
    pub fn hairline(&self) -> f32 {
        (1.0 / self.ppp).max(self.s.min(1.0) / self.ppp)
    }

    /// Corner radius in points, clamped to what epaint can express.
    pub fn radius(&self, design: f32) -> f32 {
        (design * self.s).max(0.0)
    }
}

/// Vertical scroll state of a clipped region.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll {
    pub offset: f32,
    pub content: f32,
    pub view: f32,
}

impl Scroll {
    pub const WHEEL_STEP: f32 = 40.0;

    pub fn max_offset(&self) -> f32 {
        (self.content - self.view).max(0.0)
    }

    pub fn clamp(&mut self) {
        self.offset = self.offset.clamp(0.0, self.max_offset());
    }

    /// Apply a wheel delta (points, positive = scroll up) and clamp.
    pub fn wheel(&mut self, dy: f32) {
        self.offset -= dy;
        self.clamp();
    }

    pub fn scrollable(&self) -> bool {
        self.content > self.view + 0.5
    }

    /// Thumb rectangle for a vertical bar occupying `track`.
    pub fn thumb(&self, track: Rect) -> Rect {
        if !self.scrollable() {
            return Rect::NOTHING;
        }
        let frac = (self.view / self.content).clamp(0.05, 1.0);
        let h = (track.height() * frac).max(8.0);
        let t = if self.max_offset() > 0.0 { self.offset / self.max_offset() } else { 0.0 };
        let y = track.min.y + (track.height() - h) * t;
        Rect::from_min_size(Pos2::new(track.min.x, y), Vec2::new(track.width(), h))
    }
}

/// Column-fill placement for draggable widgets.
///
/// `obstacles` are the rects of already-placed, visible widgets; `size` the new
/// widget's size; `start` the preferred top-left (default `(6, 6)`).
pub fn non_overlapping_position(screen: Rect, obstacles: &[Rect], size: Vec2, start: Option<Pos2>) -> Pos2 {
    let padding = 6.0;
    let screen_size = Vec2::new((screen.width() - 100.0).max(1.0), (screen.height() - 100.0).max(1.0));
    let start = start.unwrap_or(Pos2::new(6.0, 6.0));
    let size = if size.x <= 0.0 { Vec2::new(150.0, 40.0) } else { size };
    let mut cur = start;
    let mut max_x_in_column = 0.0f32;
    let overlapping = |pos: Pos2| -> Option<Rect> {
        let r = Rect::from_min_size(pos, size);
        obstacles.iter().copied().find(|o| o.intersects(r) && o.width() > 0.0 && o.height() > 0.0)
    };
    let mut guard = 0;
    while let Some(obs) = overlapping(cur) {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        max_x_in_column = max_x_in_column.max(obs.width());
        let next_y = obs.max.y + padding;
        if next_y + size.y > screen_size.y - padding {
            let next_x = cur.x + max_x_in_column + padding;
            if next_x + size.x > screen_size.x - padding {
                break;
            }
            cur.x = next_x;
            cur.y = start.y;
            max_x_in_column = 0.0;
        } else {
            cur.y = next_y;
        }
    }
    Pos2::new(cur.x.round(), cur.y.round())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_stacks_then_moves_to_next_column() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 200.0));
        let size = Vec2::new(100.0, 40.0);
        let a = non_overlapping_position(screen, &[], size, None);
        assert_eq!(a, Pos2::new(6.0, 6.0));
        let ra = Rect::from_min_size(a, size);
        let b = non_overlapping_position(screen, &[ra], size, None);
        assert_eq!(b, Pos2::new(6.0, 52.0));
        let rb = Rect::from_min_size(b, size);
        // screen usable height is 100: third widget would end at 138 > 94 -> next column
        let c = non_overlapping_position(screen, &[ra, rb], size, None);
        assert_eq!(c, Pos2::new(112.0, 6.0));
    }

    #[test]
    fn scroll_clamps() {
        let mut s = Scroll { offset: 0.0, content: 300.0, view: 100.0 };
        s.wheel(-1000.0);
        assert_eq!(s.offset, 200.0);
        s.wheel(5000.0);
        assert_eq!(s.offset, 0.0);
        assert!(s.scrollable());
    }
}
