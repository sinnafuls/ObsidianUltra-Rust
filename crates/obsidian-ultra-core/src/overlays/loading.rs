//! Loading screen.

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, Layer};
use crate::handles::info::LoadingInfo;
use crate::handles::Loading;
use crate::input::MouseButton;
use crate::node::*;
use crate::tween::{info as tw, Anim};
use crate::ui::{Model, Pending, Ui};
use crate::window::record_overlay;

impl Ui {
    /// Create the loading screen; returns the existing one if active.
    pub fn create_loading(&self, info: LoadingInfo) -> Loading {
        let id = self.mutate(|m| {
            if let Some(l) = m.loading {
                return l;
            }
            let sidebar = m.insert(Kind::LoadingSidebar, None);
            let hid_window = m.open;
            if hid_window {
                crate::window::toggle(m, Some(false));
            }
            let data = LoadingData {
                message: String::new(),
                description: String::new(),
                current_step: info.current_step.min(info.total_steps.max(1)),
                total_steps: info.total_steps.max(1),
                show_sidebar: info.show_sidebar,
                auto_resize_height: info.auto_resize_height,
                window_width: info.window_width,
                window_height: info.window_height,
                base_height: info.window_height,
                content_width: info.content_width,
                sidebar_width: info.sidebar_width,
                is_error: false,
                error_message: String::new(),
                error_buttons: Vec::new(),
                sidebar,
                pos: Pos2::ZERO,
                placed: false,
                w: Anim::new(0.0),
                h: Anim::new(0.0),
                step: Anim::new(0.0),
                spinner_start: m.time,
                loading_icon: info.loading_icon.clone(),
                loading_icon_color: info.loading_icon_color,
                spin_time: info.loading_icon_tween_time.max(0.0),
                hid_window,
                info,
            };
            let id = m.insert(Kind::Loading(Box::new(data)), None);
            m.nodes[sidebar].parent = Some(id);
            m.loading = Some(id);
            id
        });
        Loading { ui: self.clone(), id }
    }
}

/// `Loading:Destroy` / `Continue`.
pub(crate) fn destroy(m: &mut Model, id: NodeId) {
    let hid = match &m.nodes[id].kind { Kind::Loading(l) => l.hid_window, _ => false };
    m.destroy_subtree(id);
    if m.loading == Some(id) {
        m.loading = None;
    }
    if hid && !m.open && !m.unloaded && m.window.is_some() {
        crate::window::toggle(m, Some(true));
    }
}

const TOPBAR: f32 = 48.0;

pub(crate) fn pass(m: &mut Model, id: NodeId, cx: &mut Cx) {
    let Some(Kind::Loading(l)) = m.nodes.get(id).map(|n| &n.kind) else { return };
    let (title, icon, icon_size) = (l.info.title.clone(), l.info.icon.clone(), l.info.icon_size);
    let (message, description, cur, total, show_sidebar, auto_resize) = (l.message.clone(), l.description.clone(), l.current_step, l.total_steps, l.show_sidebar, l.auto_resize_height);
    let (window_w, base_h, content_w, sidebar_w) = (l.window_width, l.base_height, l.content_width, l.sidebar_width);
    let (is_error, error_message) = (l.is_error, l.error_message.clone());
    let error_buttons: Vec<(String, crate::types::ButtonVariant)> = l.error_buttons.iter().map(|b| (b.title.clone(), b.variant)).collect();
    let (sidebar, placed, spinner_start, loading_icon, icon_color, spin_time) = (l.sidebar, l.placed, l.spinner_start, l.loading_icon.clone(), l.loading_icon_color, l.spin_time);
    let screen = m.screen;
    let s = cx.m.s;

    // Content measurements (design px) for auto-resize / error height.
    let content_w_px = cx.px(if show_sidebar { content_w } else { window_w });
    let wrap = if auto_resize { Some(content_w_px - cx.px(60.0)) } else { None };
    let msg_sz = if message.is_empty() { Vec2::ZERO } else { cx.text_size(&message, 18.0, wrap, true) };
    let desc_sz = if description.is_empty() { Vec2::ZERO } else { cx.text_size(&description, 14.0, wrap, true) };
    let mut inner_h = cx.px(64.0);
    if msg_sz.y > 0.0 {
        inner_h += cx.px(12.0) + msg_sz.y;
    }
    if desc_sz.y > 0.0 {
        inner_h += cx.px(12.0) + desc_sz.y;
    }
    inner_h += cx.px(12.0 + 15.0);
    let mut window_h = base_h;
    if auto_resize {
        window_h = window_h.max(49.0 + 48.0 + inner_h / s);
    }
    let error_widths: Vec<f32> = error_buttons.iter().map(|(t, _)| crate::overlays::dialog::button_width(cx, t)).collect();
    let err_text_h = if is_error { cx.text_size(&error_message, 14.0, Some(content_w_px - cx.px(30.0)), true).y } else { 0.0 };
    let error_h = 49.0 + 15.0 + 18.0 + 6.0 + err_text_h / s + 15.0 + if error_buttons.is_empty() { 0.0 } else { 48.0 };

    let target_w = if show_sidebar { content_w + sidebar_w } else { window_w };
    let target_h = if is_error { error_h } else { window_h };
    if let Kind::Loading(l) = &mut m.nodes[id].kind {
        l.window_height = window_h;
    }

    // Tweened size, first-frame placement (centered).
    let (w, h) = {
        let Kind::Loading(l) = &mut m.nodes[id].kind else { return };
        let (w, h) = if !placed {
            l.w.snap(target_w);
            l.h.snap(target_h);
            (target_w, target_h)
        } else {
            let mut a = l.w;
            let w = cx.tween(&mut a, target_w, tw::DIALOG);
            l.w = a;
            let mut b = l.h;
            let h = cx.tween(&mut b, target_h, tw::DIALOG);
            l.h = b;
            (w, h)
        };
        let size = Vec2::new(cx.px(w), cx.px(h));
        if !placed {
            l.pos = Pos2::new(screen.center().x - size.x / 2.0, screen.center().y - size.y / 2.0);
            l.placed = true;
        }
        (size.x, size.y)
    };

    // Drag by the top bar.
    let pos = match &m.nodes[id].kind {
        Kind::Loading(l) => l.pos,
        _ => return,
    };
    let top_bar = Rect::from_min_size(pos, Vec2::new(w, cx.px(TOPBAR)));
    if m.capture.is_none() && cx.clicked(top_bar) && !m.settings.cant_drag_forced {
        if let Some(p) = cx.pointer() {
            m.capture = Some(Capture { node: id, sub: cap::LOADING_DRAG, start: p, data: [pos.x, pos.y, 0.0, 0.0], since: cx.time });
        }
    }
    let pos = match m.capture {
        Some(c) if c.node == id && c.sub == cap::LOADING_DRAG => {
            let p = cx.pointer().unwrap_or(c.start);
            let d = p - c.start;
            let np = Pos2::new(c.data[0] + d.x, c.data[1] + d.y);
            if let Kind::Loading(l) = &mut m.nodes[id].kind {
                l.pos = np;
            }
            cx.animate();
            np
        }
        _ => pos,
    };
    let rect = Rect::from_min_size(pos, Vec2::new(w, h));
    m.nodes[id].rect = rect;
    record_overlay(m, Layer::Window, rect);

    let corners = Corners::same(cx.r());
    let font_c = cx.col(cx.sch.font_color, 0.0);
    let outline_c = cx.col(cx.sch.outline, 0.0);
    cx.rect(rect, corners, cx.col(cx.sch.better(cx.sch.background, -1.0), 0.0));

    cx.with_clip(rect, |cx| {
        // Top bar: icon + title.
        let mut tx = rect.min.x + cx.px(12.0);
        let cy = rect.min.y + cx.px(TOPBAR / 2.0);
        if let Some(i) = &icon {
            let ir = Rect::from_center_size(Pos2::new(tx + cx.px(icon_size / 2.0), cy), Vec2::splat(cx.px(icon_size)));
            if cx.icon(i, ir, font_c) {
                tx += cx.px(icon_size + 6.0);
            }
        }
        let tr = Rect::from_min_max(Pos2::new(tx, rect.min.y), Pos2::new(rect.min.x + content_w_px.min(w) - cx.px(12.0), rect.min.y + cx.px(TOPBAR)));
        let t = cx.truncate(&title, 20.0, tr.width().max(1.0), true);
        cx.with_clip(tr, |cx| {
            cx.text(tr, &t, 20.0, font_c, Align::Min, false, true);
        });
        cx.hline(rect.min.x, rect.max.x, rect.min.y + cx.px(TOPBAR), outline_c);

        let content = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + cx.px(TOPBAR + 1.0)), Pos2::new(rect.min.x + content_w_px.min(w), rect.max.y));
        if is_error {
            // Error page.
            let x = content.min.x + cx.px(15.0);
            let iw = content.width() - cx.px(30.0);
            let title_r = Rect::from_min_size(Pos2::new(x, content.min.y + cx.px(15.0)), Vec2::new(iw, cx.px(18.0)));
            cx.text(title_r, "Error", 18.0, cx.col(cx.sch.red, 0.0), Align::Min, false, false);
            let msg_r = Rect::from_min_size(Pos2::new(x, content.min.y + cx.px(39.0)), Vec2::new(iw, err_text_h.max(1.0)));
            cx.text_top(msg_r, &error_message, 14.0, cx.col(cx.sch.font_color, 0.2), Align::Min, true);
            if !error_buttons.is_empty() {
                cx.hline(x, x + iw, content.max.y - cx.px(48.0), outline_c);
                let by = content.max.y - cx.px(15.0 + 26.0);
                let mut bx = content.max.x - cx.px(15.0);
                let mut clicked = None;
                for (i, (btitle, variant)) in error_buttons.iter().enumerate().rev() {
                    let bw = error_widths[i];
                    bx -= bw;
                    let r = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(bw, cx.px(26.0)));
                    let mut hover = match &m.nodes[id].kind {
                        Kind::Loading(l) => l.error_buttons.get(i).map(|b| b.hover).unwrap_or_default(),
                        _ => Anim::default(),
                    };
                    if crate::overlays::dialog::draw_variant_button(cx, r, btitle, *variant, &mut hover, false, None) {
                        clicked = Some(i);
                    }
                    if let Kind::Loading(l) = &mut m.nodes[id].kind {
                        if let Some(b) = l.error_buttons.get_mut(i) {
                            b.hover = hover;
                            b.rect = r;
                        }
                    }
                    bx -= cx.px(8.0);
                }
                if let Some(i) = clicked {
                    m.pending.push(Pending::ErrorButton(id, i));
                }
            }
        } else {
            // Centered content column: spinner, message, description, progress bar.
            let mut y = content.min.y + ((content.height() - inner_h) / 2.0).max(0.0);
            let cxm = content.center().x;
            let spinner = Rect::from_center_size(Pos2::new(cxm, y + cx.px(32.0)), Vec2::splat(cx.px(64.0)));
            let angle = if spin_time > 0.0 {
                cx.animate();
                (((cx.time - spinner_start) / spin_time as f64) as f32 * std::f32::consts::TAU).rem_euclid(std::f32::consts::TAU)
            } else {
                0.0
            };
            cx.icon_rotated(&loading_icon, spinner, cx.col(icon_color.unwrap_or(cx.sch.accent), 0.0), angle);
            y += cx.px(64.0);
            if msg_sz.y > 0.0 {
                y += cx.px(12.0);
                let r = Rect::from_min_size(Pos2::new(cxm - msg_sz.x / 2.0, y), msg_sz);
                cx.text_top(r, &message, 18.0, font_c, Align::Min, true);
                y += msg_sz.y;
            }
            if desc_sz.y > 0.0 {
                y += cx.px(12.0);
                let r = Rect::from_min_size(Pos2::new(cxm - desc_sz.x / 2.0, y), desc_sz);
                cx.text_top(r, &description, 14.0, cx.col(cx.sch.font_color, 0.5), Align::Min, true);
                y += desc_sz.y;
            }
            y += cx.px(12.0);
            let bar_w = content.width() * 0.7;
            let bar = Rect::from_min_size(Pos2::new(cxm - bar_w / 2.0, y), Vec2::new(bar_w, cx.px(15.0)));
            let progress = {
                let Kind::Loading(l) = &mut m.nodes[id].kind else { return };
                let target = cur as f32 / total.max(1) as f32;
                let mut a = l.step;
                let v = cx.tween(&mut a, target, tw::DIALOG);
                l.step = a;
                v
            };
            let bc = Corners::same(cx.r2());
            cx.rect(bar, bc, cx.col(cx.sch.main, 0.0));
            if progress > 0.0 {
                let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * progress.clamp(0.0, 1.0), bar.height()));
                cx.with_clip(fill, |cx| cx.rect(bar, bc, cx.col(cx.sch.accent, 0.0)));
            }
            cx.stroke(bar, bc, 1.0, outline_c, StrokeKind::Inside);
            cx.text(bar, &format!("{cur}/{total}"), 14.0, font_c, Align::Center, false, false);
        }

        // Sidebar.
        if show_sidebar && sidebar_w > 0.0 {
            let sb = Rect::from_min_max(Pos2::new(rect.min.x + content_w_px, rect.min.y), rect.max);
            cx.vline(sb.min.x, sb.min.y, sb.max.y, outline_c);
            let children = m.nodes[sidebar].children.clone();
            cx.with_clip(sb, |cx| {
                let inner = sb.shrink(cx.px(12.0));
                crate::elements::pass_children(m, &children, cx, inner.min.x, inner.min.y, inner.width(), cx.px(8.0), false);
            });
            cx.outline(sb, corners, 0.0);
        }
    });
    cx.outline(rect, corners, 0.0);
    if !cx.down(MouseButton::Left) {
        if let Some(c) = m.capture {
            if c.node == id && c.sub == cap::LOADING_DRAG {
                m.capture = None;
            }
        }
    }
}
