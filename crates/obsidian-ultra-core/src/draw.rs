//! Shape builders for the Obsidian look: rounded rects, outlines, gradients,
//! procedural textures (checkerboard, SV map, hue bar), glow and the crosshair cursor.

use std::f32::consts::{FRAC_PI_2, PI};

use epaint::{Color32, CornerRadius, Mesh, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, WHITE_UV};

use crate::color;

/// Per-corner radii in points (design radius already scaled).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Corners {
    pub nw: f32,
    pub ne: f32,
    pub sw: f32,
    pub se: f32,
}

impl Corners {
    pub const ZERO: Corners = Corners { nw: 0.0, ne: 0.0, sw: 0.0, se: 0.0 };

    pub fn same(r: f32) -> Self {
        Self { nw: r, ne: r, sw: r, se: r }
    }
    pub fn top(r: f32) -> Self {
        Self { nw: r, ne: r, sw: 0.0, se: 0.0 }
    }
    pub fn bottom(r: f32) -> Self {
        Self { nw: 0.0, ne: 0.0, sw: r, se: r }
    }
    pub fn no_left(r: f32) -> Self {
        Self { nw: 0.0, ne: r, sw: 0.0, se: r }
    }
    pub fn no_top_left(r: f32) -> Self {
        Self { nw: 0.0, ne: r, sw: r, se: r }
    }
    pub fn no_bottom_left(r: f32) -> Self {
        Self { nw: r, ne: r, sw: 0.0, se: r }
    }
    pub fn pill(rect: Rect) -> Self {
        Self::same(rect.height().min(rect.width()) / 2.0)
    }

    fn clamp(self, rect: Rect) -> Self {
        let m = (rect.width().min(rect.height()) / 2.0).max(0.0);
        Self { nw: self.nw.min(m), ne: self.ne.min(m), sw: self.sw.min(m), se: self.se.min(m) }
    }

    pub fn to_epaint(self) -> CornerRadius {
        CornerRadius {
            nw: self.nw.round().clamp(0.0, 255.0) as u8,
            ne: self.ne.round().clamp(0.0, 255.0) as u8,
            sw: self.sw.round().clamp(0.0, 255.0) as u8,
            se: self.se.round().clamp(0.0, 255.0) as u8,
        }
    }
}

impl From<f32> for Corners {
    fn from(r: f32) -> Self {
        Corners::same(r)
    }
}

/// Filled rounded rectangle.
pub fn rect(rect: Rect, corners: impl Into<Corners>, fill: Color32) -> Shape {
    if fill.a() == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Shape::Noop;
    }
    Shape::rect_filled(rect, corners.into().clamp(rect).to_epaint(), fill)
}

/// Stroked rounded rectangle; `kind` chooses where the stroke sits relative to `rect`.
pub fn stroke(rect: Rect, corners: impl Into<Corners>, width: f32, color: Color32, kind: StrokeKind) -> Shape {
    if color.a() == 0 || width <= 0.0 {
        return Shape::Noop;
    }
    Shape::rect_stroke(rect, corners.into().clamp(rect).to_epaint(), Stroke::new(width, color), kind)
}

/// A 1px `outline` stroke on the border plus a 1.5px `dark` shadow stroke outside it.
pub fn outline(rect: Rect, corners: Corners, outline: Color32, dark: Color32, s: f32, out: &mut Vec<Shape>) {
    let sw = 1.5 * s;
    let bigger = Corners { nw: corners.nw + sw, ne: corners.ne + sw, sw: corners.sw + sw, se: corners.se + sw };
    out.push(stroke(rect.expand(0.0), bigger, sw, dark, StrokeKind::Outside));
    out.push(stroke(rect, corners, 1.0 * s, outline, StrokeKind::Inside));
}

/// Horizontal line of `thickness` points.
pub fn hline(x0: f32, x1: f32, y: f32, thickness: f32, color: Color32) -> Shape {
    rect(Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + thickness)), 0.0, color)
}

/// Vertical line of `thickness` points.
pub fn vline(x: f32, y0: f32, y1: f32, thickness: f32, color: Color32) -> Shape {
    rect(Rect::from_min_max(Pos2::new(x, y0), Pos2::new(x + thickness, y1)), 0.0, color)
}

/// Outline path of a rounded rectangle (clockwise, starting at the top-left arc).
fn rounded_path(rect: Rect, corners: Corners, segs: usize) -> Vec<Pos2> {
    let c = corners.clamp(rect);
    let mut pts = Vec::with_capacity(4 * (segs + 1));
    let arc = |pts: &mut Vec<Pos2>, center: Pos2, r: f32, a0: f32| {
        if r <= 0.0 {
            pts.push(center);
            return;
        }
        for i in 0..=segs {
            let a = a0 + FRAC_PI_2 * i as f32 / segs as f32;
            pts.push(Pos2::new(center.x + r * a.cos(), center.y + r * a.sin()));
        }
    };
    arc(&mut pts, Pos2::new(rect.min.x + c.nw, rect.min.y + c.nw), c.nw, PI);
    arc(&mut pts, Pos2::new(rect.max.x - c.ne, rect.min.y + c.ne), c.ne, -FRAC_PI_2);
    arc(&mut pts, Pos2::new(rect.max.x - c.se, rect.max.y - c.se), c.se, 0.0);
    arc(&mut pts, Pos2::new(rect.min.x + c.sw, rect.max.y - c.sw), c.sw, FRAC_PI_2);
    pts
}

/// Rounded rectangle filled with a linear gradient; `horizontal` runs `from` (left/top) -> `to` (right/bottom).
/// Exact for linear gradients: the color is linear in x (or y) and the polygon is convex.
pub fn gradient_rounded_rect(r: Rect, corners: Corners, from: Color32, to: Color32, horizontal: bool) -> Shape {
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return Shape::Noop;
    }
    let path = rounded_path(r, corners, 6);
    let mut mesh = Mesh::default();
    let center = r.center();
    let color_at = |p: Pos2| {
        let t = if horizontal { (p.x - r.min.x) / r.width() } else { (p.y - r.min.y) / r.height() };
        color::lerp(from, to, t)
    };
    mesh.colored_vertex(center, color_at(center));
    for p in &path {
        mesh.colored_vertex(*p, color_at(*p));
    }
    let n = path.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    Shape::mesh(mesh)
}

/// Axis-aligned gradient rectangle without rounding.
pub fn gradient_rect(r: Rect, from: Color32, to: Color32, horizontal: bool) -> Shape {
    gradient_rounded_rect(r, Corners::ZERO, from, to, horizontal)
}

/// Checkerboard (Roblox `TransparencyTexture`) with `cell`-sized squares.
pub fn checkerboard(r: Rect, cell: f32, out: &mut Vec<Shape>) {
    let light = Color32::from_gray(200);
    let dark = Color32::from_gray(120);
    out.push(rect(r, 0.0, light));
    if cell <= 0.5 {
        return;
    }
    let mut mesh = Mesh::default();
    let cols = (r.width() / cell).ceil() as usize;
    let rows = (r.height() / cell).ceil() as usize;
    for row in 0..rows {
        for col in 0..cols {
            if (row + col) % 2 == 0 {
                continue;
            }
            let x0 = r.min.x + col as f32 * cell;
            let y0 = r.min.y + row as f32 * cell;
            let cr = Rect::from_min_max(Pos2::new(x0, y0), Pos2::new((x0 + cell).min(r.max.x), (y0 + cell).min(r.max.y)));
            mesh.add_colored_rect(cr, dark);
        }
    }
    out.push(Shape::mesh(mesh));
}

/// Saturation/value map for a hue: white->hue horizontally, then black overlay vertically.
pub fn sv_map(r: Rect, hue: f32, out: &mut Vec<Shape>) {
    let hue_color = color::from_hsv(hue, 1.0, 1.0);
    out.push(gradient_rect(r, Color32::WHITE, hue_color, true));
    out.push(gradient_rect(r, Color32::TRANSPARENT, Color32::BLACK, false));
}

/// Vertical hue bar with 11 keypoints (`fromHSV(h,1,1)` for h = 0, 0.1, …, 1).
pub fn hue_bar(r: Rect, out: &mut Vec<Shape>) {
    let steps = 10;
    for i in 0..steps {
        let t0 = i as f32 / steps as f32;
        let t1 = (i + 1) as f32 / steps as f32;
        let seg = Rect::from_min_max(
            Pos2::new(r.min.x, r.min.y + r.height() * t0),
            Pos2::new(r.max.x, r.min.y + r.height() * t1),
        );
        out.push(gradient_rect(seg, color::from_hsv(t0, 1.0, 1.0), color::from_hsv(t1, 1.0, 1.0), false));
    }
}

/// Alpha bar: checkerboard under the color fading to transparent from top to bottom.
pub fn alpha_bar(r: Rect, color: Color32, out: &mut Vec<Shape>) {
    checkerboard(r, 8.0, out);
    out.push(gradient_rect(r, color, Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0), false));
}

/// Soft glow around `r`: 8 concentric rounded strokes with linearly falling alpha.
pub fn glow(r: Rect, corners: Corners, radius: f32, color: Color32, transparency: f32, out: &mut Vec<Shape>) {
    if radius <= 0.0 {
        return;
    }
    let rings = 8;
    let w = radius / rings as f32;
    for i in 0..rings {
        let inset = w * i as f32;
        let alpha = (1.0 - transparency) * (1.0 - i as f32 / rings as f32) * 0.5;
        let c = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.0) as u8);
        let rr = r.expand(inset + w * 0.5);
        let cc = Corners { nw: corners.nw + inset, ne: corners.ne + inset, sw: corners.sw + inset, se: corners.se + inset };
        out.push(stroke(rr, cc, w, c, StrokeKind::Middle));
    }
}

/// Textured quad with tint.
pub fn image(tex: epaint::TextureId, r: Rect, uv: Rect, tint: Color32) -> Shape {
    if tint.a() == 0 {
        return Shape::Noop;
    }
    Shape::image(tex, r, uv, tint)
}

/// A textured quad rotated by `angle` radians around the rect center.
pub fn image_rotated(tex: epaint::TextureId, r: Rect, uv: Rect, tint: Color32, angle: f32) -> Shape {
    if angle.abs() < 1e-4 {
        return image(tex, r, uv, tint);
    }
    let mut mesh = Mesh::with_texture(tex);
    mesh.add_rect_with_uv(r, uv, tint);
    mesh.rotate(epaint::emath::Rot2::from_angle(angle), r.center());
    Shape::mesh(mesh)
}

/// The Obsidian crosshair cursor: 9×1 + 1×9 white bars with a 1px dark outline.
pub fn crosshair(center: Pos2, s: f32, white: Color32, dark: Color32, out: &mut Vec<Shape>) {
    let h = |w: f32, hgt: f32, c: Color32| rect(Rect::from_center_size(center, Vec2::new(w * s, hgt * s)), 0.0, c);
    out.push(h(11.0, 3.0, dark));
    out.push(h(3.0, 11.0, dark));
    out.push(h(9.0, 1.0, white));
    out.push(h(1.0, 9.0, white));
}

/// Full-UV rect for untextured meshes.
pub const FULL_UV: Rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));

/// UV for solid-color quads.
pub const SOLID_UV: Pos2 = WHITE_UV;
