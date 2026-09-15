//! Color picker addon (swatch + HSV popup + copy/paste context menu).

use epaint::emath::Align;
use epaint::{Color32, Pos2, Rect, StrokeKind, Vec2};

use crate::color::{self, Rgba};
use crate::draw::{self, Corners};
use crate::frame::{Cx, FocusRequest, Layer, TextFieldRequest};
use crate::handles::info::ColorPickerInfo;
use crate::input::MouseButton;
use crate::node::*;
use crate::tween::info as tw;
use crate::types::IconRef;
use crate::ui::{Model, Pending};

use super::menu::{self, MenuCorners, MenuSpec};

const SWATCH: f32 = 18.0;
const PAD: f32 = 6.0;
const GAP: f32 = 8.0;
const ROW: f32 = 20.0;
const TITLE_H: f32 = 8.0;
const FOOTER_H: f32 = 22.0;
const BASE_MAP: f32 = 200.0;
const BASE_BAR: f32 = 16.0;
const MIN_MAP: f32 = 140.0;
const SCREEN_MARGIN: f32 = 12.0;
const CTX_W: f32 = 93.0;
const CTX_ROW: f32 = 21.0;
const CTX_PAD: f32 = 6.0;
const FEEDBACK_SECS: f64 = 1.0;
/// Text-field sub ids.
const FIELD_HEX: u8 = 1;
const FIELD_RGB: u8 = 2;
/// Menu sub ids.
const MENU_COLOR: u8 = 0;
const MENU_CTX: u8 = 1;

pub(crate) fn create(m: &mut Model, host: NodeId, idx: &str, info: ColorPickerInfo) -> NodeId {
    let (h, s, v) = color::to_hsv(info.default);
    let data = ColorPickerData {
        hue: h,
        sat: s,
        vib: v,
        transparency: info.transparency.unwrap_or(0.0).clamp(0.0, 1.0),
        has_alpha: info.transparency.is_some(),
        title: info.title,
        resizable: info.resizable,
        listeners: info.callback.into_iter().collect(),
        menu: MenuState::NONE,
        ctx_menu: MenuState::NONE,
        map_w: BASE_MAP,
        map_h: BASE_MAP,
        default: Rgba { color: info.default, transparency: info.transparency.unwrap_or(0.0) },
        feedback: None,
        hex_focus: Default::default(),
        rgb_focus: Default::default(),
        resize_start: None,
    };
    let id = m.insert(Kind::ColorPicker(Box::new(data)), Some(host));
    match &mut m.nodes[host].kind {
        Kind::Toggle(t) => t.addons.push(id),
        Kind::Label(l) => l.addons.push(id),
        _ => {}
    }
    m.register_option(idx, id);
    id
}

fn data(m: &Model, id: NodeId) -> Option<&ColorPickerData> {
    match m.nodes.get(id) {
        Some(n) if !n.destroyed => match &n.kind {
            Kind::ColorPicker(c) => Some(c),
            _ => None,
        },
        _ => None,
    }
}

fn data_mut(m: &mut Model, id: NodeId) -> Option<&mut ColorPickerData> {
    match m.nodes.get_mut(id).map(|n| &mut n.kind) {
        Some(Kind::ColorPicker(c)) => Some(c),
        _ => None,
    }
}

/// Fire listeners with the current value.
fn fire(m: &mut Model, id: NodeId) {
    if let Some(c) = data(m, id) {
        let v = c.value();
        m.pending.push(Pending::Color(id, v));
    }
}
pub(crate) fn set_value(m: &mut Model, id: NodeId, color: Color32, transparency: Option<f32>) {
    let (h, s, v) = color::to_hsv(color);
    set_hsv(m, id, h, s, v, transparency);
}
pub(crate) fn set_hsv(m: &mut Model, id: NodeId, h: f32, s: f32, v: f32, transparency: Option<f32>) {
    let Some(c) = data_mut(m, id) else { return };
    c.hue = h.clamp(0.0, 1.0);
    c.sat = s.clamp(0.0, 1.0);
    c.vib = v.clamp(0.0, 1.0);
    if c.has_alpha {
        if let Some(t) = transparency {
            c.transparency = t.clamp(0.0, 1.0);
        }
    } else {
        c.transparency = 0.0;
    }
    fire(m, id);
}

// ----- geometry ------------------------------------------------------------------------------

/// Hue/alpha bar width in design px.
fn bar_width(map_w: f32) -> f32 {
    (map_w / BASE_MAP * BASE_BAR).round().clamp(12.0, 24.0)
}

/// Color menu width in design px.
fn content_width(map_w: f32, has_alpha: bool) -> f32 {
    let bar = bar_width(map_w);
    let mut w = map_w + bar + PAD;
    if has_alpha {
        w += bar + PAD;
    }
    w + 2.0 * PAD
}

/// Menu height minus the map height, in design px.
fn vertical_overhead(has_title: bool) -> f32 {
    let mut h = PAD + PAD + GAP + ROW + GAP + ROW + FOOTER_H;
    if has_title {
        h += TITLE_H + GAP;
    }
    h
}

/// Cursor size in design px.
fn cursor_size(map_w: f32, map_h: f32) -> f32 {
    (map_w.min(map_h) / BASE_MAP * 6.0).round().clamp(4.0, 10.0)
}

/// Round, clamp to the minimum and to the screen (`menu_min` in points).
fn update_size(m: &mut Model, id: NodeId, cx: &Cx, new_w: f32, new_h: f32, menu_min: Pos2) {
    let Some(c) = data(m, id) else { return };
    let (has_alpha, has_title) = (c.has_alpha, c.title.is_some());
    let mut w = (new_w + 0.5).floor().max(MIN_MAP);
    let mut h = (new_h + 0.5).floor().max(MIN_MAP);
    let screen = cx.fi.screen;
    let max_w = (screen.max.x - menu_min.x) / cx.m.s - SCREEN_MARGIN;
    let max_h = (screen.max.y - menu_min.y) / cx.m.s - SCREEN_MARGIN - vertical_overhead(has_title);
    while w > MIN_MAP && content_width(w, has_alpha) > max_w {
        w -= 4.0;
    }
    if h > max_h {
        h = max_h.floor().max(MIN_MAP);
    }
    if let Some(c) = data_mut(m, id) {
        c.map_w = w;
        c.map_h = h;
    }
}

// ----- swatch --------------------------------------------------------------------------------

/// Draw the 18×18 swatch right-aligned at `right_x`, centered on `cy`. Returns the width used.
pub(crate) fn pass_addon(m: &mut Model, id: NodeId, cx: &mut Cx, right_x: f32, cy: f32) -> f32 {
    let Some(c) = data(m, id) else { return 0.0 };
    let (has_alpha, menu_open, ctx_open) = (c.has_alpha, c.menu.open, c.ctx_menu.open);
    let value = c.value();
    let size = cx.px(SWATCH);
    let r = Rect::from_min_size(Pos2::new((right_x - size).floor(), (cy - size / 2.0).floor()), Vec2::splat(size));
    m.nodes[id].rect = r;

    let r2 = cx.r2();
    let corners = Corners {
        nw: r2,
        ne: if ctx_open { 0.0 } else { r2 },
        sw: if menu_open { 0.0 } else { r2 },
        se: if menu_open || ctx_open { 0.0 } else { r2 },
    };
    if has_alpha {
        let mut v = Vec::with_capacity(2);
        draw::checkerboard(r, cx.px(9.0), &mut v);
        cx.with_clip(r, |cx| cx.push_all(v));
    }
    cx.rect(r, corners, cx.col(value.color, value.transparency));
    cx.stroke(r, corners, 1.0, cx.col(color::darker(value.color), 0.0), StrokeKind::Inside);

    if cx.clicked(r) {
        toggle_menu(m, id, MENU_COLOR);
    } else if cx.right_clicked(r) {
        toggle_menu(m, id, MENU_CTX);
    }

    pass_color_menu(m, id, cx, r);
    pass_context_menu(m, id, cx, r);
    size
}

fn menu_state(m: &Model, id: NodeId, sub: u8) -> Option<MenuState> {
    data(m, id).map(|c| if sub == MENU_COLOR { c.menu } else { c.ctx_menu })
}

fn store_menu(m: &mut Model, id: NodeId, sub: u8, st: MenuState) {
    if let Some(c) = data_mut(m, id) {
        if sub == MENU_COLOR {
            c.menu = st;
        } else {
            c.ctx_menu = st;
        }
    }
}

fn toggle_menu(m: &mut Model, id: NodeId, sub: u8) {
    let Some(mut st) = menu_state(m, id, sub) else { return };
    menu::toggle(m, id, sub, &mut st);
    store_menu(m, id, sub, st);
}

fn close_menu(m: &mut Model, id: NodeId, sub: u8) {
    let Some(mut st) = menu_state(m, id, sub) else { return };
    menu::close(m, id, sub, &mut st);
    store_menu(m, id, sub, st);
}

// ----- color menu ----------------------------------------------------------------------------

/// Copy the value into the library clipboard.
fn copy_color(m: &mut Model, id: NodeId) {
    if let Some(c) = data(m, id) {
        m.copied_color = Some(c.value());
    }
}

/// Paste from the library clipboard; returns false when nothing was copied yet.
fn paste_color(m: &mut Model, id: NodeId) -> bool {
    let Some(copied) = m.copied_color else { return false };
    set_value(m, id, copied.color, Some(copied.transparency));
    true
}

fn set_feedback(m: &mut Model, id: NodeId, cx: &Cx, button: u8, text: &str) {
    if let Some(c) = data_mut(m, id) {
        c.feedback = Some((button, text.to_owned(), cx.time + FEEDBACK_SECS));
    }
}

/// A `MainColor` button with hover `better(main, 10)`; returns true when clicked.
fn menu_button(cx: &mut Cx, r: Rect, text: &str) -> bool {
    let hovered = cx.hovered(r);
    let bg = if hovered { cx.sch.better(cx.sch.main, 10.0) } else { cx.sch.main };
    cx.rect(r, cx.r2(), cx.col(bg, 0.0));
    cx.stroke(r, cx.r2(), 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Inside);
    let t = cx.truncate(text, 14.0, r.width() - cx.px(4.0), false);
    cx.text(r, &t, 14.0, cx.col(cx.sch.font_color, 0.0), Align::Center, false, false);
    hovered && cx.pressed(MouseButton::Left)
}

/// A hosted text field in a `MainColor` box whose stroke tweens to the accent while focused.
/// Returns the submitted text, if any.
fn menu_field(m: &mut Model, id: NodeId, cx: &mut Cx, r: Rect, sub: u8, text: &str) -> Option<String> {
    let resp = cx.field(id, sub).cloned();
    let focused = resp.as_ref().map(|f| f.focused).unwrap_or(false);
    let fv = {
        let Some(c) = data_mut(m, id) else { return None };
        let mut a = if sub == FIELD_HEX { c.hex_focus } else { c.rgb_focus };
        let v = cx.tween(&mut a, if focused { 1.0 } else { 0.0 }, tw::HOVER);
        if sub == FIELD_HEX {
            c.hex_focus = a;
        } else {
            c.rgb_focus = a;
        }
        v
    };
    cx.rect(r, cx.r2(), cx.col(cx.sch.main, 0.0));
    let stroke_c = color::lerp(cx.sch.outline, cx.sch.accent, fv);
    cx.stroke(r, cx.r2(), 1.0, cx.col(stroke_c, 0.0), StrokeKind::Inside);
    let inner = r.shrink2(Vec2::new(cx.px(4.0), 0.0));
    let color = cx.col(cx.sch.font_color, 0.0);
    if m.open && cx.fade > 0.99 && !crate::window::covered(m, cx.layer, r) {
        cx.request_field(TextFieldRequest {
            id,
            sub,
            rect: inner,
            text: text.to_owned(),
            placeholder: String::new(),
            font: cx.font(14.0),
            color,
            placeholder_color: cx.col(cx.sch.placeholder(), 0.0),
            focus: FocusRequest::None,
            numeric: false,
            max_len: None,
            clear_on_focus: false,
            password: false,
            layer: cx.layer,
        });
    } else {
        cx.text(inner, text, 14.0, color, Align::Center, false, false);
    }
    resp.filter(|f| f.submitted).map(|f| f.text)
}

/// Start or continue a pointer capture on `rect`; returns the clamped pointer when active.
fn drag(m: &mut Model, id: NodeId, cx: &Cx, sub: u8, rect: Rect) -> Option<Pos2> {
    let active = m.capture.map(|c| c.node == id && c.sub == sub).unwrap_or(false);
    if !active {
        if m.capture.is_none() && cx.clicked(rect) {
            m.capture = Some(Capture { node: id, sub, start: cx.pointer()?, data: [0.0; 4], since: cx.time });
        } else {
            return None;
        }
    }
    let p = cx.pointer()?;
    Some(Pos2::new(p.x.clamp(rect.min.x, rect.max.x), p.y.clamp(rect.min.y, rect.max.y)))
}

fn pass_color_menu(m: &mut Model, id: NodeId, cx: &mut Cx, holder: Rect) {
    let Some(c) = data(m, id) else { return };
    let (has_alpha, title, resizable, map_w, map_h) = (c.has_alpha, c.title.clone(), c.resizable, c.map_w, c.map_h);
    let Some(mut st) = menu_state(m, id, MENU_COLOR) else { return };
    let width = cx.px(content_width(map_w, has_alpha));
    let height = cx.px(map_h + vertical_overhead(title.is_some()));
    let spec = MenuSpec {
        holder,
        width,
        offset: Vec2::new(cx.px(0.5), holder.height() + cx.px(1.5)),
        height,
        anim: None,
    };
    let rect = menu::begin(m, id, MENU_COLOR, cx, &mut st, spec);
    store_menu(m, id, MENU_COLOR, st);
    let Some(mr) = rect else {
        // Menu closed: drop any resize marker left over from a drag.
        if data(m, id).map(|c| c.resize_start.is_some()).unwrap_or(false) {
            if let Some(c) = data_mut(m, id) {
                c.resize_start = None;
            }
            m.settings.cant_drag_forced = false;
        }
        return;
    };

    cx.with_layer(Layer::Menus, mr, false, |cx| {
        menu::chrome(cx, mr, MenuCorners::NoTopLeft);
        let pad = cx.px(PAD);
        let gap = cx.px(GAP);
        let x0 = mr.min.x + pad;
        let inner_w = mr.width() - 2.0 * pad;
        let mut y = mr.min.y + pad;

        if let Some(t) = &title {
            let tr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(inner_w, cx.px(TITLE_H)));
            cx.text(tr, t, 14.0, cx.col(cx.sch.font_color, 0.0), Align::Min, false, true);
            y += cx.px(TITLE_H) + gap;
        }

        // Color holder: SV map, hue bar, optional alpha bar.
        let bar_w = cx.px(bar_width(map_w));
        let map = Rect::from_min_size(Pos2::new(x0, y), cx.m.v(map_w, map_h));
        let hue = Rect::from_min_size(Pos2::new(map.max.x + pad, y), Vec2::new(bar_w, map.height()));
        let alpha = Rect::from_min_size(Pos2::new(hue.max.x + pad, y), Vec2::new(bar_w, map.height()));

        // Drags (update the value only when it changed).
        let Some(c) = data(m, id) else { return };
        let (h0, s0, v0, t0) = (c.hue, c.sat, c.vib, c.transparency);
        let mut next = (h0, s0, v0, t0);
        if let Some(p) = drag(m, id, cx, cap::SV, map) {
            next.1 = ((p.x - map.min.x) / map.width()).clamp(0.0, 1.0);
            next.2 = 1.0 - ((p.y - map.min.y) / map.height()).clamp(0.0, 1.0);
        } else if let Some(p) = drag(m, id, cx, cap::HUE, hue) {
            next.0 = ((p.y - hue.min.y) / hue.height()).clamp(0.0, 1.0);
        } else if has_alpha {
            if let Some(p) = drag(m, id, cx, cap::ALPHA, alpha) {
                next.3 = ((p.y - alpha.min.y) / alpha.height()).clamp(0.0, 1.0);
            }
        }
        if next != (h0, s0, v0, t0) {
            set_hsv(m, id, next.0, next.1, next.2, Some(next.3));
        }
        let Some(c) = data(m, id) else { return };
        let (hue_v, sat, vib, transparency) = (c.hue, c.sat, c.vib, c.transparency);
        let value = c.value();

        let mut shapes = Vec::with_capacity(16);
        draw::sv_map(map, hue_v, &mut shapes);
        draw::hue_bar(hue, &mut shapes);
        if has_alpha {
            draw::alpha_bar(alpha, value.color, &mut shapes);
        }
        for s in shapes.iter_mut() {
            fade_shape(s, cx.fade);
        }
        cx.push_all(shapes);

        // Cursors.
        let cur = cx.px(cursor_size(map_w, map_h)) / 2.0;
        let sv_c = Pos2::new(map.min.x + map.width() * sat, map.min.y + map.height() * (1.0 - vib));
        cx.push(epaint::Shape::circle_filled(sv_c, cur, cx.col(cx.sch.white, 0.0)));
        cx.push(epaint::Shape::circle_stroke(sv_c, cur, epaint::Stroke::new(cx.px(1.0), cx.col(cx.sch.dark, 0.0))));
        bar_cursor(cx, hue, hue_v);
        if has_alpha {
            bar_cursor(cx, alpha, transparency);
        }
        y += map.height() + gap;

        // Info holder: hex + rgb fields.
        let bw = (inner_w - gap) / 2.0;
        let row_h = cx.px(ROW);
        let hex_r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(bw, row_h));
        let rgb_r = Rect::from_min_size(Pos2::new(x0 + bw + gap, y), Vec2::new(bw, row_h));
        let hex = color::to_hex(value.color);
        let rgb = color::to_rgb_string(value.color);
        if let Some(text) = menu_field(m, id, cx, hex_r, FIELD_HEX, &hex) {
            if let Some(col) = color::parse_hex(&text) {
                set_value(m, id, col, None);
            }
        }
        if let Some(text) = menu_field(m, id, cx, rgb_r, FIELD_RGB, &rgb) {
            if let Some(col) = color::parse_rgb(&text) {
                set_value(m, id, col, None);
            }
        }
        y += row_h + gap;

        // Action holder: copy / paste with 1s feedback.
        let feedback = match data_mut(m, id) {
            Some(c) => {
                if c.feedback.as_ref().map(|f| cx.time >= f.2).unwrap_or(false) {
                    c.feedback = None;
                }
                c.feedback.clone()
            }
            None => None,
        };
        if feedback.is_some() {
            cx.animate();
        }
        let label = |button: u8, default: &str| -> String {
            match &feedback {
                Some((b, text, _)) if *b == button => text.clone(),
                _ => default.to_owned(),
            }
        };
        let copy_r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(bw, row_h));
        let paste_r = Rect::from_min_size(Pos2::new(x0 + bw + gap, y), Vec2::new(bw, row_h));
        if menu_button(cx, copy_r, &label(0, "Copy color")) {
            copy_color(m, id);
            set_feedback(m, id, cx, 0, "Copied color");
        }
        if menu_button(cx, paste_r, &label(1, "Paste color")) {
            let text = if paste_color(m, id) { "Pasted color" } else { "Nothing to paste" };
            set_feedback(m, id, cx, 1, text);
        }

        // Footer.
        let footer = Rect::from_min_size(Pos2::new(mr.min.x, mr.max.y - cx.px(FOOTER_H)), Vec2::new(mr.width(), cx.px(FOOTER_H)));
        cx.rect(footer, Corners::bottom(cx.r2()), cx.col(cx.sch.better(cx.sch.background, 4.0), 0.0));
        cx.hline(footer.min.x, footer.max.x, footer.min.y, cx.col(cx.sch.outline, 0.0));
        let right_pad = cx.px(if resizable { FOOTER_H + 4.0 } else { PAD });
        let info_r = Rect::from_min_max(Pos2::new(footer.min.x + pad, footer.min.y), Pos2::new(footer.max.x - right_pad, footer.max.y));
        let value = data(m, id).map(|c| c.value()).unwrap_or(value);
        let info = format!("{} • {}", color::to_hex(value.color), color::to_rgb_string(value.color));
        let info = cx.truncate(&info, 14.0, info_r.width(), false);
        cx.text(info_r, &info, 14.0, cx.col(cx.sch.font_color, 0.5), Align::Center, false, false);

        if resizable {
            let g = cx.px(FOOTER_H);
            let grab = Rect::from_min_size(Pos2::new(footer.max.x - g - cx.r() / 4.0, footer.min.y), Vec2::splat(g));
            cx.icon(&IconRef::Lucide("move-diagonal-2".into()), grab.shrink(cx.px(2.0)), cx.col(cx.sch.font_color, 0.5));
            let capturing = m.capture.map(|c| c.node == id && c.sub == cap::CP_RESIZE).unwrap_or(false);
            if !capturing && m.capture.is_none() && cx.clicked(grab) {
                if let Some(p) = cx.pointer() {
                    m.capture = Some(Capture { node: id, sub: cap::CP_RESIZE, start: p, data: [map_w, map_h, 0.0, 0.0], since: cx.time });
                    if let Some(c) = data_mut(m, id) {
                        c.resize_start = Some((p, map_w, map_h));
                    }
                    m.settings.cant_drag_forced = true;
                }
            }
            match (m.capture, data(m, id).and_then(|c| c.resize_start)) {
                (Some(cap), Some((start, w0, h0))) if cap.node == id && cap.sub == cap::CP_RESIZE => {
                    if let Some(p) = cx.pointer() {
                        let d = (p - start) / cx.m.s;
                        update_size(m, id, cx, w0 + d.x, h0 + d.y, mr.min);
                        cx.animate();
                    }
                }
                (_, Some(_)) => {
                    if let Some(c) = data_mut(m, id) {
                        c.resize_start = None;
                    }
                    m.settings.cant_drag_forced = false;
                }
                _ => {}
            }
        }
    });
}

/// 1px `WhiteColor` line with a 1px `DarkColor` border across a bar at fraction `t`.
fn bar_cursor(cx: &mut Cx, bar: Rect, t: f32) {
    let y = cx.m.snap(bar.min.y + bar.height() * t.clamp(0.0, 1.0));
    let one = cx.px(1.0).max(cx.m.hairline());
    let outer = Rect::from_min_max(Pos2::new(bar.min.x - one, y - one), Pos2::new(bar.max.x + one, y + 2.0 * one));
    cx.rect(outer, 0.0, cx.col(cx.sch.dark, 0.0));
    let inner = Rect::from_min_max(Pos2::new(bar.min.x - one, y), Pos2::new(bar.max.x + one, y + one));
    cx.rect(inner, 0.0, cx.col(cx.sch.white, 0.0));
}

/// Apply the window fade to a procedural shape (meshes and rects).
fn fade_shape(s: &mut epaint::Shape, fade: f32) {
    if fade >= 0.999 {
        return;
    }
    match s {
        epaint::Shape::Mesh(mesh) => {
            for v in std::sync::Arc::make_mut(mesh).vertices.iter_mut() {
                v.color = color::fade(v.color, fade);
            }
        }
        epaint::Shape::Rect(r) => r.fill = color::fade(r.fill, fade),
        _ => {}
    }
}

// ----- right-click context menu -------------------------------------------------------------

fn pass_context_menu(m: &mut Model, id: NodeId, cx: &mut Cx, holder: Rect) {
    let Some(mut st) = menu_state(m, id, MENU_CTX) else { return };
    const ENTRIES: [&str; 4] = ["Copy color", "Paste color", "Copy Hex", "Copy RGB"];
    let row_h = cx.px(CTX_ROW);
    let pad = cx.px(CTX_PAD);
    let spec = MenuSpec {
        holder,
        width: cx.px(CTX_W),
        offset: Vec2::new(holder.width() + cx.px(1.5), cx.px(0.5)),
        height: row_h * ENTRIES.len() as f32 + 2.0 * pad,
        anim: None,
    };
    let rect = menu::begin(m, id, MENU_CTX, cx, &mut st, spec);
    store_menu(m, id, MENU_CTX, st);
    let Some(mr) = rect else { return };
    let open = st.open;
    cx.with_layer(Layer::Menus, mr, false, |cx| {
        menu::chrome(cx, mr, MenuCorners::NoTopLeft);
        let mut picked = None;
        for (i, label) in ENTRIES.iter().enumerate() {
            let row = Rect::from_min_size(Pos2::new(mr.min.x, mr.min.y + pad + row_h * i as f32), Vec2::new(mr.width(), row_h));
            let hovered = open && cx.hovered(row);
            if hovered {
                cx.rect(row, 0.0, cx.col(cx.sch.main, 0.7));
            }
            cx.text(row, label, 14.0, cx.col(cx.sch.font_color, 0.0), Align::Center, false, false);
            if hovered && cx.pressed(MouseButton::Left) {
                picked = Some(i);
            }
        }
        let Some(i) = picked else { return };
        let value = match data(m, id) {
            Some(c) => c.value(),
            None => return,
        };
        match i {
            0 => copy_color(m, id),
            1 => {
                paste_color(m, id);
            }
            2 => cx.out.copy_to_clipboard.push(color::to_hex(value.color)),
            _ => cx.out.copy_to_clipboard.push(color::to_rgb_string(value.color)),
        }
        close_menu(m, id, MENU_CTX);
    });
}
