//! Floats: draggable label/button/menu/image-button, watermark, keybind frame, pop-out floats.
//! Pop-outs delegate to `window::popout`.

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, Layer};
use crate::handles::info::*;
use crate::handles::*;
use crate::input::MouseButton;
use crate::node::*;
use crate::types::{IconPosition, IconRef};
use crate::ui::{Model, Pending, Ui};
use crate::window::record_overlay;

impl Ui {
    pub fn add_draggable_label(&self, info: DraggableLabelInfo) -> DraggableLabel {
        let id = self.with(|m| {
            let id = m.insert(Kind::DraggableLabel(DraggableLabelData { text: info.text, icon: info.icon, icon_position: info.icon_position, pos: Pos2::new(6.0, 6.0), placed: false, z: 0 }), None);
            register_float(m, id);
            id
        });
        DraggableLabel { ui: self.clone(), id }
    }

    pub fn add_draggable_button(&self, info: DraggableButtonInfo) -> DraggableButton {
        let id = self.with(|m| {
            let id = m.insert(
                Kind::DraggableButton(DraggableButtonData { text: info.text, callback: info.callback.into_iter().collect(), exclude_scaling: info.exclude_scaling, exclude_dragging: info.exclude_dragging, pos: Pos2::new(6.0, 6.0), placed: false, z: 0, press_since: None }),
                None,
            );
            register_float(m, id);
            id
        });
        DraggableButton { ui: self.clone(), id }
    }

    pub fn add_draggable_menu(&self, name: &str) -> DraggableMenu {
        let id = self.with(|m| create_draggable_menu(m, name, false));
        DraggableMenu { ui: self.clone(), id }
    }

    pub fn add_draggable_image_button(&self, info: DraggableImageButtonInfo) -> DraggableImageButton {
        let id = self.with(|m| {
            let id = m.insert(
                Kind::DraggableImageButton(DraggableImageButtonData { icon: info.icon, icon_size: info.icon_size.max(1.0), callback: info.callback.into_iter().collect(), exclude_scaling: info.exclude_scaling, exclude_dragging: info.exclude_dragging, pos: Pos2::new(6.0, 6.0), placed: false, z: 0, press_since: None }),
                None,
            );
            register_float(m, id);
            id
        });
        DraggableImageButton { ui: self.clone(), id }
    }

    pub fn add_watermark(&self, segments: Vec<WatermarkSegment>) -> Watermark {
        let id = self.with(|m| create_watermark(m, segments));
        Watermark { ui: self.clone(), id }
    }

    /// The built-in (initially hidden) watermark.
    pub fn watermark(&self) -> Watermark {
        let id = self.with(|m| m.watermark.expect("watermark exists"));
        Watermark { ui: self.clone(), id }
    }

    /// The keybind frame (a draggable menu titled "Keybinds").
    pub fn keybind_frame(&self) -> DraggableMenu {
        let id = self.with(|m| m.keybind_frame.expect("keybind frame exists"));
        DraggableMenu { ui: self.clone(), id }
    }
}

pub(crate) fn register_float(m: &mut Model, id: NodeId) {
    let z = m.bump_z();
    match &mut m.nodes[id].kind {
        Kind::DraggableLabel(d) => d.z = z,
        Kind::DraggableButton(d) => d.z = z,
        Kind::DraggableImageButton(d) => d.z = z,
        Kind::DraggableMenu(d) => d.z = z,
        Kind::Watermark(d) => d.z = z,
        _ => {}
    }
    m.floats.push(id);
}

pub(crate) fn create_draggable_menu(m: &mut Model, name: &str, is_keybind_frame: bool) -> NodeId {
    let id = m.insert(Kind::DraggableMenu(DraggableMenuData { name: name.to_owned(), pos: Pos2::new(6.0, 6.0), placed: false, z: 0, is_keybind_frame }), None);
    register_float(m, id);
    id
}

pub(crate) fn create_watermark(m: &mut Model, segments: Vec<WatermarkSegment>) -> NodeId {
    let id = m.insert(Kind::Watermark(WatermarkData { segments, texts: Vec::new(), refresh_rate: 1.0, last_refresh: 0.0, pos: Pos2::new(6.0, 6.0), placed: false, z: 0 }), None);
    register_float(m, id);
    refresh_watermark(m, id);
    id
}

/// Re-evaluate getters.
pub(crate) fn refresh_watermark(m: &mut Model, id: NodeId) {
    let now = m.time;
    let Some(Kind::Watermark(w)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    w.texts = w
        .segments
        .iter()
        .map(|s| match (&s.getter, &s.text) {
            (Some(g), _) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(g)).unwrap_or_default(),
            (None, Some(t)) => t.clone(),
            (None, None) => String::new(),
        })
        .collect();
    w.last_refresh = now;
}

/// Draw one float (any float kind).
pub(crate) fn pass(m: &mut Model, id: NodeId, cx: &mut Cx) {
    match &m.nodes[id].kind {
        Kind::Groupbox(_) | Kind::Tabbox(_) => crate::window::popout::pass_float(m, id, cx),
        Kind::DraggableLabel(_) | Kind::DraggableButton(_) | Kind::DraggableImageButton(_) | Kind::DraggableMenu(_) | Kind::Watermark(_) => {
            if !m.nodes[id].shown() {
                m.nodes[id].rect = Rect::NOTHING;
                return;
            }
            let exclude_scaling = match &m.nodes[id].kind {
                Kind::DraggableButton(b) => b.exclude_scaling,
                Kind::DraggableImageButton(b) => b.exclude_scaling,
                _ => false,
            };
            let prev_s = cx.m.s;
            if exclude_scaling {
                cx.m.s = 1.0;
            }
            pass_widget(m, id, cx);
            cx.m.s = prev_s;
        }
        _ => {}
    }
}

fn float_pos(m: &Model, id: NodeId) -> (Pos2, bool) {
    match &m.nodes[id].kind {
        Kind::DraggableLabel(d) => (d.pos, d.placed),
        Kind::DraggableButton(d) => (d.pos, d.placed),
        Kind::DraggableImageButton(d) => (d.pos, d.placed),
        Kind::DraggableMenu(d) => (d.pos, d.placed),
        Kind::Watermark(d) => (d.pos, d.placed),
        _ => (Pos2::ZERO, true),
    }
}

fn set_float_pos(m: &mut Model, id: NodeId, pos: Pos2) {
    match &mut m.nodes[id].kind {
        Kind::DraggableLabel(d) => (d.pos, d.placed) = (pos, true),
        Kind::DraggableButton(d) => (d.pos, d.placed) = (pos, true),
        Kind::DraggableImageButton(d) => (d.pos, d.placed) = (pos, true),
        Kind::DraggableMenu(d) => (d.pos, d.placed) = (pos, true),
        Kind::Watermark(d) => (d.pos, d.placed) = (pos, true),
        _ => {}
    }
}

/// First-frame placement avoiding the other visible floats.
fn place(m: &mut Model, id: NodeId, cx: &Cx, size: Vec2) -> Pos2 {
    let (pos, placed) = float_pos(m, id);
    if placed {
        return pos;
    }
    let obstacles: Vec<Rect> = m
        .floats
        .iter()
        .filter(|f| **f != id)
        .filter_map(|f| m.nodes.get(*f))
        .filter(|n| n.shown() && n.rect.is_positive())
        .map(|n| n.rect)
        .collect();
    let start = Pos2::new(m.screen.min.x + cx.px(6.0), m.screen.min.y + cx.px(6.0));
    let pos = crate::layout::non_overlapping_position(m.screen, &obstacles, size, Some(start));
    set_float_pos(m, id, pos);
    pos
}

/// Drag by `handle`; returns the current position.
/// `press` reports whether the handle was pressed this frame (buttons use it for click detection).
fn drag(m: &mut Model, id: NodeId, cx: &mut Cx, handle: Rect, pos: Pos2) -> (Pos2, bool) {
    let mut pos = pos;
    let mut pressed = false;
    if m.capture.is_none() && cx.clicked(handle) && !m.settings.cant_drag_forced {
        if let Some(p) = cx.pointer() {
            m.raise_float(id);
            m.capture = Some(Capture { node: id, sub: cap::FLOAT_DRAG, start: p, data: [pos.x, pos.y, 0.0, 0.0], since: cx.time });
            pressed = true;
        }
    }
    if let Some(c) = m.capture {
        if c.node == id && c.sub == cap::FLOAT_DRAG {
            if let Some(p) = cx.pointer() {
                let d = p - c.start;
                pos = Pos2::new(c.data[0] + d.x, c.data[1] + d.y);
                set_float_pos(m, id, pos);
            }
            cx.animate();
        }
    }
    (pos, pressed)
}

/// Press-and-release click handling for draggable buttons.
fn button_release(m: &mut Model, id: NodeId, cx: &Cx, pressed: bool, exclude_dragging: bool) -> bool {
    let ours = matches!(m.capture, Some(c) if c.node == id && c.sub == cap::FLOAT_DRAG);
    let press_since = match &mut m.nodes[id].kind {
        Kind::DraggableButton(b) => &mut b.press_since,
        Kind::DraggableImageButton(b) => &mut b.press_since,
        _ => return false,
    };
    if pressed {
        *press_since = Some(cx.time);
    }
    if ours && !cx.down(MouseButton::Left) {
        let held = press_since.map(|t| cx.time - t).unwrap_or(0.0);
        *press_since = None;
        return !(exclude_dragging && held > 0.25);
    }
    false
}

fn pass_widget(m: &mut Model, id: NodeId, cx: &mut Cx) {
    let screen = m.screen;
    let corners = Corners::same(cx.r());
    let font_c = cx.col(cx.sch.font_color, 0.0);
    let bg = cx.col(cx.sch.background, 0.0);
    let rect = match &m.nodes[id].kind {
        Kind::DraggableLabel(d) => {
            let (text, icon, icon_pos) = (d.text.clone(), d.icon.clone(), d.icon_position);
            let has_icon = icon.as_ref().map(|i| cx.resolve_icon(i, (cx.px(16.0) * cx.m.ppp) as u32).is_some()).unwrap_or(false);
            let (pad_l, pad_r) = match (has_icon, icon_pos) {
                (true, IconPosition::Left) => (34.0, 12.0),
                (true, IconPosition::Right) => (12.0, 34.0),
                _ => (12.0, 12.0),
            };
            let ts = cx.text_size(&text, 15.0, None, true);
            let size = Vec2::new(ts.x + cx.px(pad_l + pad_r), ts.y + cx.px(12.0));
            let pos = place(m, id, cx, size);
            let r = Rect::from_min_size(pos, size);
            let (pos, _) = drag(m, id, cx, r, pos);
            let r = Rect::from_min_size(pos, size);
            cx.rect(r, corners, bg);
            cx.outline(r, corners, 0.0);
            if has_icon {
                let ix = match icon_pos {
                    IconPosition::Left => r.min.x + cx.px(12.0 + 8.0),
                    IconPosition::Right => r.max.x - cx.px(12.0 + 8.0),
                };
                let ir = Rect::from_center_size(Pos2::new(ix, r.center().y), Vec2::splat(cx.px(16.0)));
                if let Some(i) = &icon {
                    cx.icon(i, ir, font_c);
                }
            }
            let tr = Rect::from_min_max(Pos2::new(r.min.x + cx.px(pad_l), r.min.y), Pos2::new(r.max.x - cx.px(pad_r), r.max.y));
            cx.text(tr, &text, 15.0, font_c, Align::Min, false, true);
            r
        }
        Kind::DraggableButton(d) => {
            let (text, exclude_dragging) = (d.text.clone(), d.exclude_dragging);
            let ts = cx.text_size(&text, 16.0, None, true);
            let size = Vec2::new(ts.x * 2.0, ts.y * 2.0);
            let pos = place(m, id, cx, size);
            let r = Rect::from_min_size(pos, size);
            let (pos, pressed) = drag(m, id, cx, r, pos);
            let r = Rect::from_min_size(pos, size);
            cx.rect(r, corners, bg);
            cx.outline(r, corners, 0.0);
            cx.text(r, &text, 16.0, font_c, Align::Center, false, true);
            if button_release(m, id, cx, pressed, exclude_dragging) {
                m.pending.push(Pending::Unit(id));
            }
            r
        }
        Kind::DraggableImageButton(d) => {
            let (icon, icon_size, exclude_dragging) = (d.icon.clone(), d.icon_size, d.exclude_dragging);
            let size = Vec2::splat(cx.px(icon_size + 12.0));
            let pos = place(m, id, cx, size);
            let r = Rect::from_min_size(pos, size);
            let (pos, pressed) = drag(m, id, cx, r, pos);
            let r = Rect::from_min_size(pos, size);
            cx.rect(r, corners, bg);
            cx.outline(r, corners, 0.0);
            let ir = Rect::from_center_size(r.center(), Vec2::splat(cx.px(icon_size)));
            cx.icon(&icon, ir, font_c);
            if button_release(m, id, cx, pressed, exclude_dragging) {
                m.pending.push(Pending::Unit(id));
            }
            r
        }
        Kind::DraggableMenu(d) => {
            let (name, is_keybind) = (d.name.clone(), d.is_keybind_frame);
            let children = m.nodes[id].children.clone();
            let title_w = cx.text_size(&name, 15.0, None, true).x;
            let rows_w = if is_keybind { crate::elements::keypicker::keybind_rows_width(m, cx) + cx.px(14.0) } else { 0.0 };
            let w = (title_w + cx.px(24.0)).max(rows_w).max(cx.px(if is_keybind { 120.0 } else { 210.0 }));
            let inner_w = w - cx.px(14.0);
            let gap = cx.px(7.0);
            let layer_idx = cx.layer as usize;
            if is_keybind {
                if !m.open {
                    m.nodes[id].rect = Rect::NOTHING;
                    return;
                }
                // Measure the rows without input (scratch draw, discarded).
                let mark = cx.out.layers[layer_idx].len();
                let prev_blocked = cx.blocked;
                cx.blocked = true;
                let rows_h = crate::elements::keypicker::pass_keybind_rows(m, cx, 0.0, 0.0, inner_w);
                cx.blocked = prev_blocked;
                cx.out.layers[layer_idx].truncate(mark);
                if rows_h <= 0.0 {
                    m.nodes[id].rect = Rect::NOTHING;
                    return;
                }
                let h = cx.px(35.0 + 14.0) + rows_h;
                let (_, placed) = float_pos(m, id);
                if !placed {
                    // Lua: anchored at the left edge, vertically centered.
                    set_float_pos(m, id, Pos2::new(screen.min.x + cx.px(6.0), screen.center().y - h / 2.0));
                }
            }
            let content_h = if is_keybind { 0.0 } else { crate::elements::measure_children(m, &children, cx, inner_w, gap) };
            let est_h = cx.px(35.0 + 14.0) + content_h;
            let pos = place(m, id, cx, Vec2::new(w, est_h));
            let title_bar = Rect::from_min_size(pos, Vec2::new(w, cx.px(34.0)));
            let (pos, _) = drag(m, id, cx, title_bar, pos);
            let title_bar = Rect::from_min_size(pos, Vec2::new(w, cx.px(34.0)));

            // Background first (rotated under the content once its height is known).
            let mark = cx.out.layers[layer_idx].len();
            let cx0 = pos.x + cx.px(7.0);
            let cy0 = pos.y + cx.px(35.0 + 7.0);
            let content_h = if is_keybind {
                crate::elements::keypicker::pass_keybind_rows(m, cx, cx0, cy0, inner_w)
            } else {
                crate::elements::pass_children(m, &children, cx, cx0, cy0, inner_w, gap, false)
            };
            let h = cx.px(35.0 + 14.0) + content_h;
            let r = Rect::from_min_size(pos, Vec2::new(w, h));
            let before = cx.out.layers[layer_idx].len();
            cx.rect(r, corners, bg);
            let n = cx.out.layers[layer_idx].len() - before;
            cx.out.layers[layer_idx][mark..].rotate_right(n);

            let tr = Rect::from_min_max(Pos2::new(title_bar.min.x + cx.px(12.0), title_bar.min.y), Pos2::new(title_bar.max.x - cx.px(12.0), title_bar.max.y));
            cx.with_clip(tr, |cx| {
                cx.text(tr, &name, 15.0, font_c, Align::Min, false, true);
            });
            cx.hline(r.min.x, r.max.x, r.min.y + cx.px(34.0), cx.col(cx.sch.outline, 0.0));
            cx.outline(r, corners, 0.0);
            r
        }
        Kind::Watermark(d) => {
            let (last, rate) = (d.last_refresh, d.refresh_rate);
            if cx.time - last >= rate.max(0.05) as f64 {
                refresh_watermark(m, id);
            }
            let Kind::Watermark(d) = &m.nodes[id].kind else { return };
            let cells: Vec<(String, bool, Option<IconRef>, Option<IconRef>)> = d
                .segments
                .iter()
                .enumerate()
                .map(|(i, seg)| (d.texts.get(i).cloned().unwrap_or_default(), seg.accent, seg.icon.clone(), seg.avatar.clone()))
                .collect();
            // Measure cells: 8px side padding, 5px gaps, avatar 18, icon 15.
            let mut widths = Vec::with_capacity(cells.len());
            let mut total = 0.0;
            for (i, (text, _, icon, avatar)) in cells.iter().enumerate() {
                let has_avatar = avatar.as_ref().map(|a| cx.resolve_icon(a, (cx.px(18.0) * cx.m.ppp) as u32).is_some()).unwrap_or(false);
                let has_icon = icon.as_ref().map(|a| cx.resolve_icon(a, (cx.px(15.0) * cx.m.ppp) as u32).is_some()).unwrap_or(false);
                let tw = cx.text_size(text, 15.0, None, true).x;
                let mut w = cx.px(16.0) + tw;
                if has_avatar {
                    w += cx.px(18.0 + 5.0);
                }
                if has_icon {
                    w += cx.px(15.0 + 5.0);
                }
                widths.push((w, has_avatar, has_icon));
                total += w + if i > 0 { cx.px(1.0) } else { 0.0 };
            }
            let size = Vec2::new(total + cx.px(6.0), cx.px(26.0));
            let pos = place(m, id, cx, size);
            let r = Rect::from_min_size(pos, size);
            let (pos, _) = drag(m, id, cx, r, pos);
            let r = Rect::from_min_size(pos, size);
            cx.rect(r, corners, bg);
            cx.outline(r, corners, 0.0);
            let accent = cx.col(cx.sch.accent, 0.0);
            let outline = cx.col(cx.sch.outline, 0.0);
            let mut x = r.min.x + cx.px(3.0);
            let cy = r.center().y;
            for (i, ((text, is_accent, icon, avatar), (w, has_avatar, has_icon))) in cells.iter().zip(widths.iter()).enumerate() {
                if i > 0 {
                    cx.vline(x, cy - cx.px(7.0), cy + cx.px(7.0), outline);
                    x += cx.px(1.0);
                }
                let tint = if *is_accent { accent } else { font_c };
                let mut ix = x + cx.px(8.0);
                if *has_avatar {
                    let ar = Rect::from_center_size(Pos2::new(ix + cx.px(9.0), cy), Vec2::splat(cx.px(18.0)));
                    if let Some(a) = avatar {
                        cx.icon(a, ar, cx.col(cx.sch.white, 0.0));
                    }
                    cx.stroke(ar, cx.pill(ar), 1.0, if *is_accent { accent } else { outline }, StrokeKind::Outside);
                    ix += cx.px(18.0 + 5.0);
                }
                if *has_icon {
                    let ir = Rect::from_center_size(Pos2::new(ix + cx.px(7.5), cy), Vec2::splat(cx.px(15.0)));
                    if let Some(a) = icon {
                        cx.icon(a, ir, tint);
                    }
                    ix += cx.px(15.0 + 5.0);
                }
                let tr = Rect::from_min_max(Pos2::new(ix, cy - cx.px(10.0)), Pos2::new(x + w - cx.px(8.0), cy + cx.px(10.0)));
                cx.text(tr, text, 15.0, tint, Align::Min, false, true);
                x += w;
            }
            r
        }
        _ => return,
    };
    m.nodes[id].rect = rect;
    record_overlay(m, Layer::Floats, rect.intersect(screen));
}
