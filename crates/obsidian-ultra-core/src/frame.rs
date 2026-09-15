//! Per-frame protocol between the core and a backend, plus the internal pass context.

use std::sync::Arc;

use epaint::text::{LayoutJob, TextFormat};
use epaint::{ClippedShape, Color32, FontId, Galley, Pos2, Rect, Shape, StrokeKind, TextureId, Vec2};

use crate::color;
use crate::draw::{self, Corners};
use crate::input::{Input, MouseButton};
use crate::layout::M;
use crate::node::NodeId;
use crate::richtext;
use crate::scheme::Scheme;
use crate::tween::Ease;
use crate::types::IconRef;

/// Paint layers, bottom to top.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// Main window, minimized card, loading screen and their in-window overlays.
    Window = 0,
    /// Pop-out floats, draggable widgets, watermark, keybind frame, history panel.
    Floats = 1,
    /// Context menus and dropdown lists.
    Menus = 2,
    Tooltip = 3,
    Notifications = 4,
    /// Window snap guides.
    Guides = 5,
    Cursor = 6,
}

impl Layer {
    pub const COUNT: usize = 7;
    pub const ALL: [Layer; 7] =
        [Layer::Window, Layer::Floats, Layer::Menus, Layer::Tooltip, Layer::Notifications, Layer::Guides, Layer::Cursor];
}

/// Text layout service provided by the backend (wraps `epaint::text::FontsView`).
pub trait TextLayout {
    fn layout(&mut self, job: LayoutJob) -> Arc<Galley>;
}

/// Icon/image resolution service provided by the backend.
pub trait IconResolver {
    /// Resolve `icon` for drawing at roughly `size_px` physical pixels.
    /// Returns the texture and its natural size in texels.
    fn resolve(&mut self, icon: &IconRef, size_px: u32) -> Option<(TextureId, Vec2)>;
}

/// Focus change requested for a text field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FocusRequest {
    #[default]
    None,
    Take,
    Release,
}

/// A text field the backend must host at `rect` this frame.
#[derive(Clone, Debug)]
pub struct TextFieldRequest {
    pub id: NodeId,
    pub sub: u8,
    pub rect: Rect,
    pub text: String,
    pub placeholder: String,
    pub font: FontId,
    pub color: Color32,
    pub placeholder_color: Color32,
    pub focus: FocusRequest,
    pub numeric: bool,
    pub max_len: Option<usize>,
    pub clear_on_focus: bool,
    pub password: bool,
    /// Vertical center alignment offset hint; the backend centers text in `rect`.
    pub layer: Layer,
}

/// What happened to a hosted text field since the last frame.
#[derive(Clone, Debug, Default)]
pub struct TextFieldResponse {
    pub id: NodeId,
    pub sub: u8,
    pub text: String,
    pub focused: bool,
    pub submitted: bool,
    pub lost_focus: bool,
    pub gained_focus: bool,
}

/// Cursor the backend should show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorRequest {
    #[default]
    Default,
    /// The core drew its own cursor; hide the OS cursor.
    Hidden,
}

/// Everything the core needs to run one frame.
pub struct FrameInput<'a> {
    pub screen: Rect,
    pub pixels_per_point: f32,
    pub time: f64,
    pub dt: f32,
    pub input: &'a Input,
    pub text_fields: &'a [TextFieldResponse],
    pub fonts: &'a mut dyn TextLayout,
    pub icons: &'a mut dyn IconResolver,
}

/// Everything the backend must do after a frame.
#[derive(Default)]
pub struct FrameOutput {
    pub layers: [Vec<ClippedShape>; Layer::COUNT],
    pub text_fields: Vec<TextFieldRequest>,
    pub cursor: CursorRequest,
    pub copy_to_clipboard: Vec<String>,
    pub repaint_after: Option<f32>,
    pub wants_pointer: bool,
    pub wants_keyboard: bool,
    /// Screen rects the host should treat as owned by Obsidian (block widgets underneath).
    pub interactive_rects: Vec<Rect>,
}

/// Tooltip requested by a hovered widget this frame.
#[derive(Clone, Debug)]
pub(crate) struct TooltipReq {
    pub node: NodeId,
    pub text: String,
}

/// Internal pass context: input, output, style and helpers shared by every pass function.
pub(crate) struct Cx<'a, 'b> {
    pub fi: &'a mut FrameInput<'b>,
    pub out: &'a mut FrameOutput,
    pub layer: Layer,
    pub clip: Rect,
    /// The pointer is over a higher layer/overlay: nothing here may react to it.
    pub blocked: bool,
    pub m: M,
    pub sch: Scheme,
    /// Design corner radius (before scaling).
    pub radius: f32,
    pub time: f64,
    pub dt: f32,
    /// Alpha multiplier for the window fade animation.
    pub fade: f32,
    pub animating: bool,
    pub tooltip: Option<TooltipReq>,
    /// Inside a key tab: labels are centered.
    pub key_tab: bool,
    /// Window content is covered by an in-window overlay (dialog / expanded panel): no hosted fields.
    pub content_covered: bool,
}

impl<'a, 'b> Cx<'a, 'b> {
    // ----- geometry / style -----------------------------------------------------------

    pub fn px(&self, v: f32) -> f32 {
        self.m.px(v)
    }

    /// Full corner radius in points.
    pub fn r(&self) -> f32 {
        self.m.radius(self.radius)
    }

    /// Half corner radius in points (inner boxes).
    pub fn r2(&self) -> f32 {
        self.m.radius(self.radius / 2.0)
    }

    /// Pill radius for `rect` (square when the scheme radius is 0).
    pub fn pill(&self, rect: Rect) -> Corners {
        if self.radius > 0.0 { Corners::pill(rect) } else { Corners::ZERO }
    }

    /// Apply Obsidian-style transparency and the window fade.
    pub fn col(&self, c: Color32, transparency: f32) -> Color32 {
        color::fade(color::with_alpha(c, transparency), self.fade)
    }

    pub fn font(&self, size: f32) -> FontId {
        FontId::new(self.px(size), self.sch.font.clone())
    }

    // ----- output -------------------------------------------------------------------

    pub fn push(&mut self, shape: Shape) {
        if matches!(shape, Shape::Noop) {
            return;
        }
        self.out.layers[self.layer as usize].push(ClippedShape { clip_rect: self.clip, shape });
    }

    pub fn push_all(&mut self, shapes: Vec<Shape>) {
        for s in shapes {
            self.push(s);
        }
    }

    pub fn rect(&mut self, r: Rect, corners: impl Into<Corners>, fill: Color32) {
        let s = draw::rect(r, corners, fill);
        self.push(s);
    }

    pub fn stroke(&mut self, r: Rect, corners: impl Into<Corners>, width_design: f32, color: Color32, kind: StrokeKind) {
        let s = draw::stroke(r, corners, self.px(width_design).max(self.m.hairline()), color, kind);
        self.push(s);
    }
    pub fn outline(&mut self, r: Rect, corners: Corners, transparency: f32) {
        let outline = self.col(self.sch.outline, transparency);
        let dark = self.col(self.sch.dark, transparency);
        let mut v = Vec::with_capacity(2);
        draw::outline(r, corners, outline, dark, self.m.s, &mut v);
        self.push_all(v);
    }

    pub fn hline(&mut self, x0: f32, x1: f32, y: f32, color: Color32) {
        let t = self.px(1.0).max(self.m.hairline());
        let s = draw::hline(x0, x1, self.m.snap(y), t, color);
        self.push(s);
    }

    pub fn vline(&mut self, x: f32, y0: f32, y1: f32, color: Color32) {
        let t = self.px(1.0).max(self.m.hairline());
        let s = draw::vline(self.m.snap(x), y0, y1, t, color);
        self.push(s);
    }

    // ----- text ---------------------------------------------------------------------

    /// Lay out text. `size` is the design font size; `wrap` the wrap width in points.
    pub fn layout(&mut self, text: &str, size: f32, color: Color32, wrap: Option<f32>, halign: epaint::emath::Align, rich: bool) -> Arc<Galley> {
        let mut job = LayoutJob::default();
        job.halign = halign;
        job.wrap.max_width = wrap.unwrap_or(f32::INFINITY);
        job.round_output_to_gui = true;
        let font = self.font(size);
        if rich && (text.contains('<') || text.contains('&')) {
            for span in richtext::parse(text) {
                let fmt = TextFormat {
                    font_id: font.clone(),
                    color: span.color.map(|c| color::fade(c, color.a() as f32 / 255.0)).unwrap_or(color),
                    underline: if span.underline { epaint::Stroke::new(1.0, color) } else { epaint::Stroke::NONE },
                    strikethrough: if span.strike { epaint::Stroke::new(1.0, color) } else { epaint::Stroke::NONE },
                    italics: span.italic,
                    ..Default::default()
                };
                job.append(&span.text, 0.0, fmt);
            }
            if job.sections.is_empty() {
                job.append("", 0.0, TextFormat { font_id: font, color, ..Default::default() });
            }
        } else {
            job.append(text, 0.0, TextFormat { font_id: font, color, ..Default::default() });
        }
        self.fi.fonts.layout(job)
    }

    /// Measured size of `text` at design `size`, optionally wrapped.
    pub fn text_size(&mut self, text: &str, size: f32, wrap: Option<f32>, rich: bool) -> Vec2 {
        let g = self.layout(text, size, Color32::WHITE, wrap, epaint::emath::Align::LEFT, rich);
        g.size()
    }

    pub fn galley(&mut self, galley: Arc<Galley>, pos: Pos2, color: Color32, bold: bool) {
        if bold {
            let shifted = Pos2::new(pos.x + 0.6 * self.m.s, pos.y);
            self.push(Shape::galley(shifted, galley.clone(), color));
        }
        self.push(Shape::galley(pos, galley, color));
    }

    /// Draw `text` inside `rect`, vertically centered, with the given horizontal alignment.
    /// Returns the laid-out size.
    pub fn text(&mut self, rect: Rect, text: &str, size: f32, color: Color32, halign: epaint::emath::Align, wrap: bool, rich: bool) -> Vec2 {
        let is_bold = rich && text.contains("<b>");
        let g = self.layout(text, size, color, if wrap { Some(rect.width()) } else { None }, halign, rich);
        let sz = g.size();
        let x = match halign {
            epaint::emath::Align::Min => rect.min.x,
            epaint::emath::Align::Center => rect.center().x,
            epaint::emath::Align::Max => rect.max.x,
        };
        let y = rect.center().y - sz.y / 2.0;
        self.galley(g, Pos2::new(x, y), color, is_bold);
        sz
    }

    /// Draw text top-aligned inside `rect` (wrapped).
    pub fn text_top(&mut self, rect: Rect, text: &str, size: f32, color: Color32, halign: epaint::emath::Align, rich: bool) -> Vec2 {
        let is_bold = rich && text.contains("<b>");
        let g = self.layout(text, size, color, Some(rect.width()), halign, rich);
        let sz = g.size();
        let x = match halign {
            epaint::emath::Align::Min => rect.min.x,
            epaint::emath::Align::Center => rect.center().x,
            epaint::emath::Align::Max => rect.max.x,
        };
        self.galley(g, Pos2::new(x, rect.min.y), color, is_bold);
        sz
    }

    /// Truncate text with `…` to fit `width` (Roblox `TextTruncate.AtEnd`).
    pub fn truncate(&mut self, text: &str, size: f32, width: f32, rich: bool) -> String {
        let full = self.text_size(text, size, None, rich);
        if full.x <= width || text.is_empty() {
            return text.to_owned();
        }
        let plain = if rich { richtext::strip(text) } else { text.to_owned() };
        let chars: Vec<char> = plain.chars().collect();
        let mut lo = 0usize;
        let mut hi = chars.len();
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let candidate: String = chars[..mid].iter().collect::<String>() + "…";
            if self.text_size(&candidate, size, None, false).x <= width {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        chars[..lo].iter().collect::<String>() + "…"
    }

    // ----- icons --------------------------------------------------------------------

    /// Draw an icon fitted into `rect` with `tint`. Returns false if it could not be resolved.
    pub fn icon(&mut self, icon: &IconRef, rect: Rect, tint: Color32) -> bool {
        self.icon_rotated(icon, rect, tint, 0.0)
    }

    pub fn icon_rotated(&mut self, icon: &IconRef, rect: Rect, tint: Color32, angle: f32) -> bool {
        let px = (rect.height().max(rect.width()) * self.m.ppp).round().max(1.0) as u32;
        let Some((tex, size)) = self.fi.icons.resolve(icon, px) else { return false };
        let fitted = fit_rect(rect, size);
        let s = draw::image_rotated(tex, fitted, draw::FULL_UV, tint, angle);
        self.push(s);
        true
    }

    /// Resolve without drawing (for images that need custom UV/scale handling).
    pub fn resolve_icon(&mut self, icon: &IconRef, px: u32) -> Option<(TextureId, Vec2)> {
        self.fi.icons.resolve(icon, px)
    }

    // ----- input --------------------------------------------------------------------

    pub fn pointer(&self) -> Option<Pos2> {
        self.fi.input.pointer
    }

    /// Pointer over `rect`, inside the clip, and not blocked by a higher overlay.
    pub fn hovered(&self, rect: Rect) -> bool {
        if self.blocked {
            return false;
        }
        match self.fi.input.pointer {
            Some(p) => rect.contains(p) && self.clip.contains(p),
            None => false,
        }
    }

    pub fn clicked(&self, rect: Rect) -> bool {
        self.hovered(rect) && self.fi.input.pressed(MouseButton::Left)
    }

    pub fn right_clicked(&self, rect: Rect) -> bool {
        self.hovered(rect) && self.fi.input.pressed(MouseButton::Right)
    }

    pub fn pressed(&self, b: MouseButton) -> bool {
        self.fi.input.pressed(b)
    }

    pub fn down(&self, b: MouseButton) -> bool {
        self.fi.input.is_down(b)
    }


    /// Scroll delta if the pointer is over `rect` (consumed by the caller).
    pub fn wheel_over(&self, rect: Rect) -> f32 {
        if self.hovered(rect) { self.fi.input.scroll.y } else { 0.0 }
    }

    // ----- text fields ---------------------------------------------------------------

    pub fn field(&self, id: NodeId, sub: u8) -> Option<&TextFieldResponse> {
        self.fi.text_fields.iter().find(|f| f.id == id && f.sub == sub)
    }

    pub fn request_field(&mut self, req: TextFieldRequest) {
        if self.content_covered {
            return;
        }
        self.out.text_fields.push(req);
    }

    // ----- animation ----------------------------------------------------------------

    /// Mark the frame as animating so the backend repaints soon.
    pub fn animate(&mut self) {
        self.animating = true;
    }

    pub fn tween(&mut self, anim: &mut crate::tween::Anim, to: f32, info: (f32, Ease)) -> f32 {
        anim.set(to, self.time, info.0, info.1);
        let v = anim.value(self.time);
        if anim.active(self.time) {
            self.animating = true;
        }
        v
    }

    pub fn anim_value(&mut self, anim: &crate::tween::Anim) -> f32 {
        if anim.active(self.time) {
            self.animating = true;
        }
        anim.value(self.time)
    }

    // ----- tooltips ------------------------------------------------------------------

    pub fn tooltip(&mut self, node: NodeId, text: &str) {
        self.tooltip = Some(TooltipReq { node, text: text.to_owned() });
    }

    // ----- clip helpers --------------------------------------------------------------

    pub fn with_clip<R>(&mut self, clip: Rect, f: impl FnOnce(&mut Self) -> R) -> R {
        let prev = self.clip;
        self.clip = prev.intersect(clip);
        let r = f(self);
        self.clip = prev;
        r
    }

    pub fn with_layer<R>(&mut self, layer: Layer, clip: Rect, blocked: bool, f: impl FnOnce(&mut Self) -> R) -> R {
        let prev = (self.layer, self.clip, self.blocked);
        self.layer = layer;
        self.clip = clip;
        self.blocked = blocked;
        let r = f(self);
        self.layer = prev.0;
        self.clip = prev.1;
        self.blocked = prev.2;
        r
    }
}

/// Fit an image of `size` texels into `rect` preserving aspect ratio (centered).
pub fn fit_rect(rect: Rect, size: Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return rect;
    }
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    let sz = size * scale;
    Rect::from_center_size(rect.center(), sz)
}

/// Cover `rect` with an image of `size` texels (crop), returning the draw rect.
pub fn cover_rect(rect: Rect, size: Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return rect;
    }
    let scale = (rect.width() / size.x).max(rect.height() / size.y);
    Rect::from_center_size(rect.center(), size * scale)
}
