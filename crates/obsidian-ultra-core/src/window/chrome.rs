//! Main window chrome: frame, title bar, search, buttons, sidebar, footer, minimized card,
//! resize/drag/snap, glow, background image, in-window overlays.

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::draw::{self, Corners};
use crate::frame::{Cx, FocusRequest, Layer, TextFieldRequest};
use crate::input::MouseButton;
use crate::node::*;
use crate::tween::{info as tw, Ease};
use crate::types::*;
use crate::ui::Model;

use super::{record_overlay, set_minimized, set_sidebar_width, tabs};

pub(crate) const TOPBAR: f32 = 48.0;
pub(crate) const BOTTOM: f32 = 20.0;

pub(crate) fn pass(m: &mut Model, win: NodeId, cx: &mut Cx) {
    if !matches!(m.nodes[win].kind, Kind::Window(_)) {
        return;
    }
    let s = cx.m.s;
    if let Kind::Window(w) = &mut m.nodes[win].kind {
        if !w.placed {
            w.placed = true;
            let screen = m.screen;
            let max = Vec2::new((screen.width() / s - 64.0).max(w.min_size.x), (screen.height() / s - 64.0).max(w.min_size.y));
            w.size = Vec2::new(w.size.x.clamp(w.min_size.x, max.x), w.size.y.clamp(w.min_size.y, max.y));
            if w.info.center {
                w.pos = Pos2::new(screen.center().x - w.size.x * s / 2.0, screen.center().y - w.size.y * s / 2.0);
                w.mini_pos = w.pos;
            }
        }
    }
    let (minimized, pos, size) = match &m.nodes[win].kind {
        Kind::Window(w) => (w.minimized, w.pos, w.size),
        _ => return,
    };
    if minimized {
        pass_mini(m, win, cx);
        return;
    }
    let rect = Rect::from_min_size(pos, size * s);
    m.nodes[win].rect = rect;
    if m.open {
        record_overlay(m, Layer::Window, rect);
    }

    // In-window overlays block the whole frame (dialogs, expanded dropdowns).
    let overlay_active = m.active_dialog.is_some() || m.active_expanded.is_some();
    let content_blocked = cx.blocked || overlay_active || !m.open;

    // Glow + background image (behind the frame).
    let corners = Corners::same(cx.r());
    let (glow, bg_image) = match &m.nodes[win].kind {
        Kind::Window(w) => (w.glow, w.background_image.clone().or_else(|| cx.sch.background_image.clone())),
        _ => (GlowConfig::default(), None),
    };
    if glow.enabled && glow.radius > 0.0 {
        let color = glow.color.unwrap_or(cx.sch.accent);
        let mut v = Vec::new();
        draw::glow(rect, corners, cx.px(glow.radius), cx.col(color, 0.0), glow.transparency, &mut v);
        cx.push_all(v);
    }
    let bg = cx.sch.better(cx.sch.background, -1.0);
    cx.rect(rect, corners, cx.col(bg, 0.0));
    if let Some(img) = bg_image {
        let tint = cx.col(epaint::Color32::WHITE, 0.75);
        if let Some((tex, _)) = cx.resolve_icon(&img, (rect.width() * cx.m.ppp) as u32) {
            let shape = draw::image(tex, rect, draw::FULL_UV, tint);
            cx.push(shape);
        }
    }

    // Interaction: drag (top bar), resize handle, sidebar grabber.
    let top_bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), cx.px(TOPBAR)));
    let radius_q = cx.px(m.corner_radius / 4.0);
    let handle_size = cx.px(BOTTOM);
    let resize_rect = Rect::from_min_size(Pos2::new(rect.max.x - radius_q - handle_size, rect.max.y - handle_size), Vec2::splat(handle_size));
    let prev_blocked = cx.blocked;
    cx.blocked = content_blocked;
    window_interactions(m, win, cx, rect, top_bar, resize_rect);
    cx.blocked = prev_blocked;

    // Re-read geometry after interactions (drag/resize may have changed it).
    let (pos, size, sidebar_w, compact, title, icon, icon_size, info_disable_search, minimizable, disable_bell, resizable, footer, copied, mini_visible) =
        match &m.nodes[win].kind {
            Kind::Window(w) => (
                w.pos,
                w.size,
                w.sidebar_width,
                w.compact,
                w.title.clone(),
                w.info.icon.clone(),
                w.info.icon_size,
                w.info.disable_search,
                w.info.minimizable,
                w.info.disable_notification_bell,
                w.info.resizable,
                w.footer.clone(),
                w.footer_copied,
                w.minimized,
            ),
            _ => return,
        };
    let _ = mini_visible;
    let rect = Rect::from_min_size(pos, size * s);
    m.nodes[win].rect = rect;
    let sidebar_px = cx.px(sidebar_w);
    let outline_c = cx.col(cx.sch.outline, 0.0);
    let font_c = cx.col(cx.sch.font_color, 0.0);

    // Top-bar separator, divider line.
    let content_top = rect.min.y + cx.px(TOPBAR + 1.0);
    let content_bottom = rect.max.y - cx.px(BOTTOM + 1.0);
    let divider_x = rect.min.x + sidebar_px;

    // Title holder: icon + title centered in the sidebar-width holder.
    {
        let holder = Rect::from_min_size(rect.min, Vec2::new(sidebar_px, cx.px(TOPBAR)));
        let icon_px = cx.px(icon_size);
        let letter = title.chars().next().unwrap_or(' ').to_string();
        let title_w = if compact { 0.0 } else { cx.text_size(&title, 20.0, None, true).x.min((holder.width() - icon_px - cx.px(18.0)).max(0.0)) };
        let has_icon = icon.is_some() || compact;
        let total = if compact { icon_px } else { title_w + if has_icon { icon_px + cx.px(6.0) } else { 0.0 } };
        let mut x = holder.center().x - total / 2.0;
        if has_icon {
            let ir = Rect::from_center_size(Pos2::new(x + icon_px / 2.0, holder.center().y), Vec2::splat(icon_px));
            let drawn = match &icon {
                Some(i) => cx.icon(i, ir, font_c),
                None => false,
            };
            if !drawn {
                cx.text(ir, &letter, icon_size * 0.6, font_c, Align::Center, false, false);
            }
            x += icon_px + cx.px(6.0);
        }
        if !compact {
            let tr = Rect::from_min_size(Pos2::new(x, holder.min.y), Vec2::new(title_w.max(1.0), holder.height()));
            let t = cx.truncate(&title, 20.0, title_w, true);
            cx.with_clip(tr.expand2(Vec2::new(2.0, 0.0)), |cx| {
                cx.text(tr, &t, 20.0, font_c, Align::Min, false, true);
            });
        }
    }

    cx.content_covered = overlay_active;
    // Right wrapper: tab info + search.
    let right_inset = cx.px(if minimizable { 28.0 } else { 0.0 } + 30.0);
    let wrapper = Rect::from_min_max(
        Pos2::new(rect.min.x + sidebar_px + cx.px(9.0), rect.min.y + cx.px(8.0)),
        Pos2::new(rect.max.x - cx.px(49.0) - right_inset, rect.min.y + cx.px(TOPBAR - 8.0)),
    );
    {
        let search_w = if info_disable_search { 0.0 } else { wrapper.width() * 0.35 };
        let info_rect = Rect::from_min_max(wrapper.min, Pos2::new(wrapper.max.x - if info_disable_search { 0.0 } else { search_w + cx.px(8.0) }, wrapper.max.y));
        tabs::pass_tab_info(m, win, cx, info_rect);
        if !info_disable_search {
            let sr = Rect::from_min_max(Pos2::new(wrapper.max.x - search_w, wrapper.min.y), wrapper.max);
            pass_search(m, win, cx, sr);
        }
    }

    // Top-right buttons: move icon (decorative), minimize, bell.
    {
        let cy = rect.min.y + cx.px(TOPBAR / 2.0);
        let move_r = Rect::from_center_size(Pos2::new(rect.max.x - cx.px(10.0 + 14.0), cy), Vec2::splat(cx.px(28.0)));
        let accent = cx.col(cx.sch.accent, 0.0);
        cx.icon(&IconRef::Lucide("move".into()), move_r, accent);
        if minimizable {
            let r = Rect::from_center_size(Pos2::new(rect.max.x - cx.px(44.0 + 12.0), cy), Vec2::splat(cx.px(24.0)));
            let hovered = cx.hovered(r) && !content_blocked;
            let h = {
                let Kind::Window(w) = &mut m.nodes[win].kind else { return };
                let a = w.minimize_hover.clone();
                let mut a = a;
                let v = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
                w.minimize_hover = a;
                v
            };
            cx.rect(r, cx.r(), cx.col(cx.sch.main, 1.0 - h));
            let ic = Rect::from_center_size(r.center(), Vec2::splat(cx.px(16.0)));
            let tint = cx.col(cx.sch.font_color, 0.35 * (1.0 - h));
            if !cx.icon(&IconRef::Lucide("minus".into()), ic, tint) {
                cx.text(ic, "-", 14.0, tint, Align::Center, false, false);
            }
            if hovered {
                cx.tooltip(win, "Minimize");
            }
            if cx.clicked(r) && !content_blocked {
                set_minimized(m, win, Some(true));
            }
        }
        if !disable_bell {
            let right = if minimizable { 72.0 } else { 42.0 };
            let r = Rect::from_center_size(Pos2::new(rect.max.x - cx.px(right + 12.0), cy), Vec2::splat(cx.px(24.0)));
            pass_bell(m, win, cx, r, false, content_blocked);
        }
    }

    // Lines.
    cx.hline(rect.min.x, rect.max.x, rect.min.y + cx.px(TOPBAR), outline_c);
    cx.vline(divider_x, rect.min.y, content_bottom, outline_c);

    // Sidebar (tab buttons) and content.
    let sidebar = Rect::from_min_max(Pos2::new(rect.min.x, content_top), Pos2::new(divider_x, content_bottom));
    let container = Rect::from_min_max(Pos2::new(divider_x + cx.px(1.0), content_top), Pos2::new(rect.max.x, content_bottom));
    cx.rect(sidebar, 0.0, cx.col(cx.sch.background, 0.0));
    cx.rect(container, 0.0, cx.col(cx.sch.better(cx.sch.background, 1.0), 0.0));
    let prev = cx.blocked;
    cx.blocked = content_blocked;
    cx.with_clip(sidebar, |cx| tabs::pass_sidebar(m, win, cx, sidebar));
    let inner = Rect::from_min_max(Pos2::new(container.min.x + cx.px(6.0), container.min.y), Pos2::new(container.max.x - cx.px(6.0), container.max.y));
    cx.with_clip(container, |cx| tabs::pass_content(m, win, cx, inner));
    cx.blocked = prev;

    // Sidebar grabber visual (hover tint on the divider).
    {
        let Kind::Window(w) = &mut m.nodes[win].kind else { return };
        let a = w.grabber_hover;
        let v = a.value(cx.time);
        if a.active(cx.time) {
            cx.animate();
        }
        w.grabber_hover = a;
        if v > 0.001 {
            let c = crate::color::lighter(cx.sch.outline);
            cx.vline(divider_x, rect.min.y, content_bottom, cx.col(c, 1.0 - v));
        }
    }

    // Bottom bar.
    let bottom_bg = Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - cx.px(BOTTOM + m.corner_radius)), rect.max);
    cx.rect(bottom_bg, Corners::bottom(cx.r()), cx.col(cx.sch.better(cx.sch.background, 4.0), 0.0));
    cx.hline(rect.min.x, rect.max.x, rect.max.y - cx.px(BOTTOM), outline_c);
    let bottom_bar = Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - cx.px(BOTTOM)), rect.max);
    pass_footer(m, win, cx, bottom_bar, &footer, copied, content_blocked);

    // Resize handle icon.
    if resizable {
        let ir = resize_rect.shrink(cx.px(2.0));
        let tint = cx.col(cx.sch.font_color, 0.5);
        cx.icon(&IconRef::Lucide("move-diagonal-2".into()), ir, tint);
    }

    // Outline on top of everything in the frame.
    cx.outline(rect, corners, 0.0);
    cx.content_covered = false;

    // In-window overlays.
    if let Some(e) = m.active_expanded {
        match &m.nodes[e].kind {
            Kind::Dropdown(_) => crate::elements::dropdown::pass_expanded(m, e, cx, rect),
            Kind::Priority(_) => crate::elements::priority::pass_expanded(m, e, cx, rect),
            _ => m.active_expanded = None,
        }
    }
    crate::overlays::dialog::pass_all(m, win, cx, rect);

    // Snap guides.
    let (gx, gy) = match &m.nodes[win].kind {
        Kind::Window(w) => (w.snap_guide_x, w.snap_guide_y),
        _ => (None, None),
    };
    if gx.is_some() || gy.is_some() {
        let screen = m.screen;
        let accent = cx.col(cx.sch.accent, 0.25);
        cx.with_layer(Layer::Guides, screen, true, |cx| {
            if let Some(x) = gx {
                cx.rect(Rect::from_min_size(Pos2::new(x - cx.px(1.0), screen.min.y), Vec2::new(cx.px(2.0), screen.height())), 0.0, accent);
            }
            if let Some(y) = gy {
                cx.rect(Rect::from_min_size(Pos2::new(screen.min.x, y - cx.px(1.0)), Vec2::new(screen.width(), cx.px(2.0))), 0.0, accent);
            }
        });
    }
}

fn window_interactions(m: &mut Model, win: NodeId, cx: &mut Cx, rect: Rect, top_bar: Rect, resize_rect: Rect) {
    let s = cx.m.s;
    let pointer = cx.pointer();
    let cap = m.capture;
    let (resizable, sidebar_resize, min_size, snapping, snap_dist, snap_margin, min_container, sidebar_w, min_sidebar, compact_w, threshold, disable_snap) =
        match &m.nodes[win].kind {
            Kind::Window(w) => (
                w.info.resizable,
                w.info.enable_sidebar_resize,
                w.min_size,
                w.snapping,
                w.snap_distance,
                w.snap_margin,
                w.info.min_container_width,
                w.sidebar_width,
                w.info.min_sidebar_width.max(64.0),
                w.info.sidebar_compact_width.max(48.0),
                w.info.sidebar_collapse_threshold.clamp(0.1, 0.9),
                w.info.disable_compacting_snap,
            ),
            _ => return,
        };
    let divider_x = rect.min.x + cx.px(sidebar_w);
    let grabber = Rect::from_min_max(Pos2::new(divider_x - cx.px(4.0), rect.min.y + cx.px(TOPBAR)), Pos2::new(divider_x + cx.px(4.0), rect.max.y - cx.px(BOTTOM)));

    // Hover state for the grabber.
    let grabber_hover = sidebar_resize && (cx.hovered(grabber) || matches!(cap, Some(c) if c.node == win && c.sub == cap::SIDEBAR));
    if let Kind::Window(w) = &mut m.nodes[win].kind {
        let mut a = w.grabber_hover;
        a.set(if grabber_hover { 1.0 } else { 0.0 }, cx.time, tw::HOVER.0, tw::HOVER.1);
        w.grabber_hover = a;
    }

    // Start captures.
    if cap.is_none() && cx.pressed(MouseButton::Left) && m.open {
        if let Some(p) = pointer {
            if sidebar_resize && cx.hovered(grabber) {
                m.capture = Some(Capture { node: win, sub: cap::SIDEBAR, start: p, data: [sidebar_w, 0.0, 0.0, 0.0], since: cx.time });
                m.settings.cant_drag_forced = true;
            } else if resizable && cx.hovered(resize_rect) {
                let size = match &m.nodes[win].kind { Kind::Window(w) => w.size, _ => Vec2::ZERO };
                m.capture = Some(Capture { node: win, sub: cap::WINDOW_RESIZE, start: p, data: [size.x, size.y, 0.0, 0.0], since: cx.time });
            } else if cx.hovered(top_bar) && !m.settings.cant_drag_forced {
                // Only start a drag if no button in the top bar took the click; buttons check `clicked`
                // after this in the same frame, so exclude their rects here.
                let cy = rect.min.y + cx.px(TOPBAR / 2.0);
                let btn_zone = Rect::from_min_max(Pos2::new(rect.max.x - cx.px(90.0), cy - cx.px(14.0)), Pos2::new(rect.max.x - cx.px(4.0), cy + cx.px(14.0)));
                let search_zone = search_rect_hint(m, win, cx, rect);
                if !btn_zone.contains(p) && !search_zone.contains(p) {
                    let pos = match &m.nodes[win].kind { Kind::Window(w) => w.pos, _ => Pos2::ZERO };
                    m.capture = Some(Capture { node: win, sub: cap::WINDOW_DRAG, start: p, data: [pos.x, pos.y, 0.0, 0.0], since: cx.time });
                }
            }
        }
    }

    let Some(c) = m.capture else { return };
    if c.node != win {
        return;
    }
    let Some(p) = pointer else { return };
    let delta = p - c.start;
    match c.sub {
        cap::WINDOW_DRAG => {
            if m.settings.cant_drag_forced {
                m.capture = None;
                return;
            }
            let mut nx = c.data[0] + delta.x;
            let mut ny = c.data[1] + delta.y;
            let mut gx = None;
            let mut gy = None;
            if snapping {
                let screen = m.screen;
                let size = rect.size();
                let margin = cx.px(snap_margin);
                let dist = cx.px(snap_dist);
                let tx = [
                    (screen.min.x + margin, "L"),
                    (screen.min.x + (screen.width() - size.x) / 2.0, "C"),
                    (screen.max.x - size.x - margin, "R"),
                ];
                let ty = [
                    (screen.min.y + margin, "T"),
                    (screen.min.y + (screen.height() - size.y) / 2.0, "C"),
                    (screen.max.y - size.y - margin, "B"),
                ];
                if let Some((v, n)) = closest(nx, &tx, dist) {
                    nx = v;
                    gx = Some(match n { "R" => v + size.x, "C" => v + size.x / 2.0, _ => v });
                }
                if let Some((v, n)) = closest(ny, &ty, dist) {
                    ny = v;
                    gy = Some(match n { "B" => v + size.y, "C" => v + size.y / 2.0, _ => v });
                }
            }
            if let Kind::Window(w) = &mut m.nodes[win].kind {
                w.pos = Pos2::new(nx, ny);
                w.snap_guide_x = gx;
                w.snap_guide_y = gy;
            }
            if !cx.down(MouseButton::Left) {
                if let Kind::Window(w) = &mut m.nodes[win].kind {
                    w.snap_guide_x = None;
                    w.snap_guide_y = None;
                }
            }
        }
        cap::WINDOW_RESIZE => {
            let screen = m.screen;
            let max_x = (screen.width() / s - 64.0).max(min_size.x);
            let max_y = (screen.height() / s - 64.0).max(min_size.y);
            let nw = (c.data[0] + delta.x / s).clamp(min_size.x, max_x.max(min_size.x));
            let nh = (c.data[1] + delta.y / s).clamp(min_size.y, max_y.max(min_size.y));
            if let Kind::Window(w) = &mut m.nodes[win].kind {
                w.size = Vec2::new(nw, nh);
            }
            // Keep the sidebar within bounds.
            let cur = match &m.nodes[win].kind { Kind::Window(w) => w.sidebar_width, _ => 0.0 };
            let max_side = nw - min_container - 1.0;
            if cur > max_side {
                set_sidebar_width(m, win, max_side);
            }
        }
        cap::SIDEBAR => {
            let width = c.data[0] + delta.x / s;
            let thr = (min_sidebar + compact_w) * threshold;
            let target = if disable_snap {
                width
            } else if width > thr {
                width.max(min_sidebar)
            } else {
                compact_w
            };
            set_sidebar_width(m, win, target);
            if !cx.down(MouseButton::Left) {
                m.settings.cant_drag_forced = false;
            }
        }
        _ => {}
    }
}

fn closest(value: f32, targets: &[(f32, &'static str)], dist: f32) -> Option<(f32, &'static str)> {
    let mut best: Option<(f32, &'static str)> = None;
    for (t, n) in targets {
        if (value - t).abs() <= dist && best.map(|(b, _)| (value - t).abs() <= (value - b).abs()).unwrap_or(true) {
            best = Some((*t, n));
        }
    }
    best
}

fn search_rect_hint(m: &Model, win: NodeId, cx: &Cx, rect: Rect) -> Rect {
    let (minimizable, sidebar_w, disable) = match &m.nodes[win].kind {
        Kind::Window(w) => (w.info.minimizable, w.sidebar_width, w.info.disable_search),
        _ => return Rect::NOTHING,
    };
    if disable {
        return Rect::NOTHING;
    }
    let right_inset = cx.px(if minimizable { 28.0 } else { 0.0 } + 30.0);
    let wrapper = Rect::from_min_max(
        Pos2::new(rect.min.x + cx.px(sidebar_w) + cx.px(9.0), rect.min.y + cx.px(8.0)),
        Pos2::new(rect.max.x - cx.px(49.0) - right_inset, rect.min.y + cx.px(TOPBAR - 8.0)),
    );
    let search_w = wrapper.width() * 0.35;
    Rect::from_min_max(Pos2::new(wrapper.max.x - search_w, wrapper.min.y), wrapper.max)
}

fn pass_search(m: &mut Model, win: NodeId, cx: &mut Cx, r: Rect) {
    let pill = cx.pill(r);
    cx.rect(r, pill, cx.col(cx.sch.main, 0.0));
    let field = cx.field(win, 0).cloned();
    let focused = field.as_ref().map(|f| f.focused).unwrap_or(false);
    let (text, focus_req, edited) = {
        let Kind::Window(w) = &mut m.nodes[win].kind else { return };
        let mut edited = None;
        if let Some(f) = &field {
            // Only a change relative to what we sent last frame is a user edit.
            if f.text != w.search_sent && f.text != w.search_text {
                w.search_text = f.text.clone();
                edited = Some(f.text.clone());
            }
        }
        let req = std::mem::replace(&mut w.search_focus, FocusRequest::None);
        w.search_sent = w.search_text.clone();
        (w.search_text.clone(), req, edited)
    };
    if let Some(t) = edited {
        crate::overlays::search::update_search(m, &t);
    }
    let stroke_c = {
        let Kind::Window(w) = &mut m.nodes[win].kind else { return };
        let mut a = w.search_focus_anim;
        let v = cx.tween(&mut a, if focused { 1.0 } else { 0.0 }, tw::HOVER);
        w.search_focus_anim = a;
        crate::color::lerp(cx.sch.outline, cx.sch.accent, v)
    };
    cx.stroke(r, pill, 1.0, cx.col(stroke_c, 0.0), StrokeKind::Inside);
    let icon_r = Rect::from_center_size(Pos2::new(r.min.x + cx.px(38.0 - 24.0 + 8.0), r.center().y), Vec2::splat(cx.px(16.0)));
    cx.icon(&IconRef::Lucide("search".into()), icon_r, cx.col(cx.sch.font_color, 0.5));
    let field_rect = Rect::from_min_max(Pos2::new(r.min.x + cx.px(38.0), r.min.y + cx.px(4.0)), Pos2::new(r.max.x - cx.px(14.0), r.max.y - cx.px(4.0)));
    let font = cx.font(15.0);
    let color = cx.col(cx.sch.font_color, 0.0);
    let placeholder_color = cx.col(cx.sch.placeholder(), 0.0);
    if m.open && cx.fade > 0.99 && !cx.content_covered && !crate::window::covered(m, Layer::Window, r) {
        cx.request_field(TextFieldRequest {
            id: win,
            sub: 0,
            rect: field_rect,
            text,
            placeholder: "Search...".to_owned(),
            font,
            color,
            placeholder_color,
            focus: focus_req,
            numeric: false,
            max_len: None,
            clear_on_focus: false,
            password: false,
            layer: Layer::Window,
        });
    } else {
        cx.text(field_rect, if text.is_empty() { "Search..." } else { &text }, 15.0, if text.is_empty() { placeholder_color } else { color }, Align::Min, false, false);
    }
    record_overlay(m, Layer::Window, r);
}

pub(crate) fn pass_bell(m: &mut Model, win: NodeId, cx: &mut Cx, r: Rect, mini: bool, blocked: bool) {
    let hovered = cx.hovered(r) && !blocked;
    let h = {
        let Kind::Window(w) = &mut m.nodes[win].kind else { return };
        let mut a = if mini { w.mini_bell_hover } else { w.bell_hover };
        let v = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
        if mini {
            w.mini_bell_hover = a;
        } else {
            w.bell_hover = a;
        }
        v
    };
    if !mini {
        cx.rect(r, cx.r(), cx.col(cx.sch.main, 1.0 - h));
    }
    let ic = Rect::from_center_size(r.center(), Vec2::splat(cx.px(16.0)));
    let tint = cx.col(cx.sch.font_color, 0.35 * (1.0 - h));
    if !cx.icon(&IconRef::Lucide("bell".into()), ic, tint) {
        cx.text(ic, "!", 14.0, tint, Align::Center, false, false);
    }
    // Badge.
    let count = m.history_unread;
    if count > 0 {
        let text = if count > 99 { "99+".to_owned() } else { count.to_string() };
        let br = Rect::from_min_size(Pos2::new(r.max.x + cx.px(2.0) - cx.px(14.0), r.min.y - cx.px(if mini { 0.0 } else { 2.0 })), Vec2::splat(cx.px(14.0)));
        let w = cx.text_size(&text, 11.0, None, false).x + cx.px(4.0);
        let br = Rect::from_min_size(Pos2::new(br.max.x - w.max(br.width()), br.min.y), Vec2::new(w.max(br.width()), br.height()));
        cx.rect(br, Corners::pill(br), cx.col(cx.sch.accent, 0.0));
        cx.text(br, &text, 11.0, cx.col(cx.sch.background, 0.0), Align::Center, false, false);
    }
    if hovered {
        cx.tooltip(win, "Notification History");
    }
    if cx.clicked(r) && !blocked {
        crate::overlays::history::set_visible(m, !m.history_open);
    }
    if mini {
        m.history_pos = Pos2::new(r.max.x, r.max.y);
    }
    // Remember the bell rect so the history panel can anchor to it.
    if !mini || m.nodes[win].rect == Rect::NOTHING {
        m.history_rest = Pos2::new(r.max.x, r.max.y);
    }
    if mini {
        m.history_rest = Pos2::new(r.max.x, r.max.y);
    }
}

fn pass_footer(m: &mut Model, win: NodeId, cx: &mut Cx, bar: Rect, footer: &Footer, copied: Option<(usize, f64)>, blocked: bool) {
    let segments: Vec<FooterSegment> = match footer {
        Footer::Text(t) => {
            let copyable = match &m.nodes[win].kind { Kind::Window(w) => w.info.copyable_footer, _ => true };
            vec![FooterSegment { text: t.clone(), copyable, copy_text: None }]
        }
        Footer::Segments(v) => v.clone(),
    };
    let gap = cx.px(6.0);
    let btn = cx.px(18.0);
    let mut widths = Vec::with_capacity(segments.len());
    let mut total = 0.0;
    for seg in &segments {
        let w = cx.text_size(&seg.text, 14.0, None, true).x + if seg.copyable { gap + btn } else { 0.0 };
        widths.push(w);
        total += w;
    }
    total += gap * segments.len().saturating_sub(1) as f32;
    let mut x = bar.center().x - total / 2.0;
    for (i, seg) in segments.iter().enumerate() {
        let color = if seg.copyable { cx.col(cx.sch.blue, 0.0) } else { cx.col(cx.sch.font_color, 0.5) };
        let tw = widths[i] - if seg.copyable { gap + btn } else { 0.0 };
        let tr = Rect::from_min_size(Pos2::new(x, bar.min.y), Vec2::new(tw, bar.height()));
        cx.text(tr, &seg.text, 14.0, color, Align::Min, false, true);
        x += tw;
        if seg.copyable {
            x += gap;
            let br = Rect::from_center_size(Pos2::new(x + btn / 2.0, bar.center().y), Vec2::splat(btn));
            let hovered = cx.hovered(br) && !blocked;
            let just_copied = copied.map(|(idx, t)| idx == i && cx.time - t < 1.5).unwrap_or(false);
            if hovered {
                cx.rect(br, cx.r(), cx.col(cx.sch.main, 0.0));
                cx.tooltip(win, "Copy to clipboard");
            }
            let ir = br.shrink(cx.px(3.0));
            let name = if just_copied { "check" } else { "copy" };
            cx.icon(&IconRef::Lucide(name.into()), ir, cx.col(cx.sch.blue, 0.0));
            if cx.clicked(br) && !blocked {
                let value = seg.copy_text.clone().unwrap_or_else(|| seg.text.clone());
                cx.out.copy_to_clipboard.push(value);
                if let Kind::Window(w) = &mut m.nodes[win].kind {
                    w.footer_copied = Some((i, cx.time));
                }
            }
            if just_copied {
                cx.animate();
            }
            x += btn;
        }
        x += gap;
    }
}

/// Minimized card.
fn pass_mini(m: &mut Model, win: NodeId, cx: &mut Cx) {
    let (pos, width, title, icon, subtitle, explicit, labels, footer, disable_bell, active_tab) = match &m.nodes[win].kind {
        Kind::Window(w) => (
            w.mini_pos,
            w.info.minimized_width,
            w.title.clone(),
            w.info.icon.clone(),
            w.mini_subtitle.clone(),
            w.mini_subtitle_explicit,
            w.mini_labels.clone(),
            w.footer.clone(),
            w.info.disable_notification_bell,
            w.active_tab,
        ),
        _ => return,
    };
    let subtitle = if explicit {
        subtitle
    } else {
        active_tab.and_then(|t| match &m.nodes[t].kind {
            Kind::Tab(td) => Some(td.name.clone()),
            Kind::KeyTab(td) => Some(td.name.clone()),
            _ => None,
        }).unwrap_or_default()
    };
    let w = cx.px(width);
    let header_h = cx.px(46.0);
    // Body labels.
    let mut label_heights = Vec::with_capacity(labels.len());
    let body_w = w - cx.px(24.0);
    for l in &labels {
        let text = match &m.nodes[*l].kind { Kind::MinimizedLabel(d) => d.text.clone(), _ => String::new() };
        let visible = m.nodes[*l].shown();
        let h = if visible { cx.text_size(&text, 13.0, Some(body_w), true).y } else { 0.0 };
        label_heights.push((text, h, visible));
    }
    let shown_labels = label_heights.iter().filter(|(_, _, v)| *v).count();
    let body_h = if shown_labels > 0 {
        label_heights.iter().filter(|(_, _, v)| *v).map(|(_, h, _)| *h).sum::<f32>() + cx.px(4.0) * (shown_labels - 1) as f32 + cx.px(10.0)
    } else {
        0.0
    };
    let footer_text = match &footer {
        Footer::Text(t) => t.clone(),
        Footer::Segments(v) => v.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" "),
    };
    let footer_h = if footer_text.is_empty() { 0.0 } else { cx.px(26.0) };
    let total_h = header_h + body_h + footer_h;
    let rect = Rect::from_min_size(pos, Vec2::new(w, total_h));
    m.nodes[win].rect = rect;
    if m.open {
        record_overlay(m, Layer::Window, rect);
    }
    let corners = Corners::same(cx.r());
    let (glow, _) = match &m.nodes[win].kind { Kind::Window(wd) => (wd.glow, ()), _ => (GlowConfig::default(), ()) };
    if glow.enabled && glow.radius > 0.0 {
        let color = glow.color.unwrap_or(cx.sch.accent);
        let mut v = Vec::new();
        draw::glow(rect, corners, cx.px(glow.radius), cx.col(color, 0.0), glow.transparency, &mut v);
        cx.push_all(v);
    }
    cx.rect(rect, corners, cx.col(cx.sch.better(cx.sch.background, -1.0), 0.0));

    // Header drag.
    let header = Rect::from_min_size(rect.min, Vec2::new(w, header_h));
    let action_zone = Rect::from_min_max(Pos2::new(rect.max.x - cx.px(70.0), header.min.y), header.max);
    if m.capture.is_none() && cx.clicked(header) && !action_zone.contains(cx.pointer().unwrap_or(Pos2::ZERO)) && m.open {
        m.capture = Some(Capture { node: win, sub: cap::MINI_DRAG, start: cx.pointer().unwrap(), data: [pos.x, pos.y, 0.0, 0.0], since: cx.time });
    }
    if let Some(c) = m.capture {
        if c.node == win && c.sub == cap::MINI_DRAG {
            if let Some(p) = cx.pointer() {
                let d = p - c.start;
                if let Kind::Window(wd) = &mut m.nodes[win].kind {
                    wd.mini_pos = Pos2::new(c.data[0] + d.x, c.data[1] + d.y);
                }
            }
        }
    }

    let font_c = cx.col(cx.sch.font_color, 0.0);
    let mut x = rect.min.x + cx.px(12.0);
    let cy = header.center().y;
    if let Some(ic) = &icon {
        let badge = Rect::from_center_size(Pos2::new(x + cx.px(13.0), cy), Vec2::splat(cx.px(26.0)));
        cx.rect(badge, cx.r().max(cx.px(2.0)), cx.col(cx.sch.main, 0.0));
        let ir = Rect::from_center_size(badge.center(), Vec2::splat(cx.px(16.0)));
        cx.icon(ic, ir, font_c);
        x += cx.px(26.0 + 10.0);
    }
    let title_right = rect.max.x - cx.px(10.0 + 22.0 * 2.0 + 10.0 * 2.0);
    let title_w = (title_right - x).max(1.0);
    let tt = cx.truncate(&format!("<b>{}</b>", title), 15.0, title_w, true);
    let has_sub = !subtitle.is_empty();
    let title_r = Rect::from_min_size(Pos2::new(x, cy - if has_sub { cx.px(15.5) } else { cx.px(8.5) }), Vec2::new(title_w, cx.px(17.0)));
    cx.text(title_r, &tt, 15.0, font_c, Align::Min, false, true);
    if has_sub {
        let st = cx.truncate(&subtitle, 12.0, title_w, false);
        let sr = Rect::from_min_size(Pos2::new(x, title_r.max.y), Vec2::new(title_w, cx.px(14.0)));
        cx.text(sr, &st, 12.0, cx.col(cx.sch.font_color, 0.55), Align::Min, false, false);
    }
    // Restore button (rightmost) and mini bell.
    let restore = Rect::from_center_size(Pos2::new(rect.max.x - cx.px(10.0 + 11.0), cy), Vec2::splat(cx.px(22.0)));
    let rh = cx.hovered(restore);
    let tint = cx.col(cx.sch.font_color, if rh { 0.0 } else { 0.35 });
    if !cx.icon(&IconRef::Lucide("chevron-up".into()), restore.shrink(cx.px(3.0)), tint) {
        cx.text(restore, "^", 14.0, tint, Align::Center, false, false);
    }
    if rh {
        cx.tooltip(win, "Restore");
    }
    if cx.clicked(restore) {
        set_minimized(m, win, Some(false));
    }
    if !disable_bell {
        let bell = Rect::from_center_size(Pos2::new(restore.min.x - cx.px(10.0 + 11.0), cy), Vec2::splat(cx.px(22.0)));
        pass_bell(m, win, cx, bell, true, false);
    }
    // Body.
    let mut y = header.max.y;
    if shown_labels > 0 {
        for (text, h, visible) in &label_heights {
            if !visible {
                continue;
            }
            let r = Rect::from_min_size(Pos2::new(rect.min.x + cx.px(12.0), y), Vec2::new(body_w, *h));
            cx.text_top(r, text, 13.0, cx.col(cx.sch.font_color, 0.25), Align::Min, true);
            y += h + cx.px(4.0);
        }
        y += cx.px(10.0) - cx.px(4.0);
    }
    if footer_h > 0.0 {
        let outline_c = cx.col(cx.sch.outline, 0.0);
        cx.hline(rect.min.x, rect.max.x, y, outline_c);
        let fr = Rect::from_min_max(Pos2::new(rect.min.x + cx.px(12.0), y), Pos2::new(rect.max.x - cx.px(12.0), y + footer_h));
        let ft = cx.truncate(&footer_text, 12.0, fr.width(), true);
        cx.text(fr, &ft, 12.0, cx.col(cx.sch.font_color, 0.6), Align::Min, false, true);
    }
    cx.outline(rect, corners, 0.0);
    // Dialogs still render over the mini card's window area? Lua parents them to MainFrame (hidden). Skip.
    let _ = Ease::Linear;
}
