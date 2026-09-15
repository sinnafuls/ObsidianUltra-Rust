//! Modal dialogs.

use epaint::emath::Align;
use epaint::{Color32, Pos2, Rect, StrokeKind, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, Layer};
use crate::handles::info::{DialogInfo, FooterButtonInfo};
use crate::node::*;
use crate::tween::{info as tw, Anim, Ease};
use crate::types::ButtonVariant;
use crate::ui::{Model, Pending};
use crate::window::record_overlay;

pub(crate) fn create(m: &mut Model, win: NodeId, idx: &str, info: DialogInfo) -> NodeId {
    let now = m.time;
    let data = DialogData {
        title: info.title,
        description: info.description,
        icon: info.icon,
        title_color: info.title_color,
        description_color: info.description_color,
        auto_dismiss: info.auto_dismiss,
        outside_click_dismiss: info.outside_click_dismiss,
        buttons: Vec::new(),
        open: Anim::new(0.0),
        closing: false,
        close_at: 0.0,
    };
    let id = m.insert(Kind::Dialog(data), Some(win));
    m.nodes[id].idx = idx.to_owned();
    if let Kind::Dialog(d) = &mut m.nodes[id].kind {
        d.open.play(0.0, 1.0, now, tw::DIALOG.0, Ease::QuadOut);
    }
    if let Kind::Window(w) = &mut m.nodes[win].kind {
        w.dialogs.push(id);
    }
    m.active_dialog = Some(id);
    for (key, b) in info.footer_buttons {
        add_button(m, id, &key, b);
    }
    id
}

pub(crate) fn add_button(m: &mut Model, dialog: NodeId, key: &str, info: FooterButtonInfo) {
    let now = m.time;
    let Some(Kind::Dialog(d)) = m.nodes.get_mut(dialog).map(|n| &mut n.kind) else { return };
    d.buttons.retain(|b| b.key != key);
    d.buttons.push(DialogButton {
        key: key.to_owned(),
        title: info.title.unwrap_or_else(|| key.to_owned()),
        variant: info.variant,
        order: info.order,
        disabled: info.wait_time.is_some(),
        wait_until: info.wait_time.map(|t| now + t as f64),
        wait_time: info.wait_time.unwrap_or(0.0),
        callback: info.callback.into_iter().collect(),
        hover: Anim::new(0.0),
        rect: Rect::NOTHING,
    });
    d.buttons.sort_by_key(|b| b.order);
}
pub(crate) fn dismiss(m: &mut Model, id: NodeId) {
    let now = m.time;
    let Some(Kind::Dialog(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    if d.closing {
        return;
    }
    d.closing = true;
    d.close_at = now + tw::DIALOG.0 as f64;
    d.open.set(0.0, now, tw::DIALOG.0, tw::DIALOG.1);
    if m.active_dialog == Some(id) {
        m.active_dialog = None;
        // Newest remaining dialog becomes active.
        if let Some(win) = m.window {
            if let Kind::Window(w) = &m.nodes[win].kind {
                m.active_dialog = w.dialogs.iter().rev().copied().find(|d| *d != id && matches!(&m.nodes[*d].kind, Kind::Dialog(dd) if !dd.closing));
            }
        }
    }
}

/// Colors of a dialog-style footer button: (background, outline, text).
fn variant_colors(cx: &Cx, v: ButtonVariant) -> (Color32, Color32, Color32) {
    match v {
        ButtonVariant::Primary => (cx.sch.font_color, cx.sch.font_color, cx.sch.background),
        ButtonVariant::Secondary => (cx.sch.main, cx.sch.outline, cx.sch.font_color),
        ButtonVariant::Destructive => (cx.sch.destructive, cx.sch.destructive, cx.sch.white),
        ButtonVariant::Ghost => (cx.sch.background, cx.sch.background, cx.sch.font_color),
    }
}

/// Width of a dialog-style button for `title`.
pub(crate) fn button_width(cx: &mut Cx, title: &str) -> f32 {
    cx.text_size(title, 14.0, None, true).x.min(cx.px(250.0)) + cx.px(30.0)
}

/// Draw a dialog-style button (also used by the loading error page). `wait` is the
/// `0..=1` progress of a wait-time bar (`None` when there is none). Returns `true` on click.
pub(crate) fn draw_variant_button(cx: &mut Cx, r: Rect, title: &str, variant: ButtonVariant, hover: &mut Anim, disabled: bool, wait: Option<f32>) -> bool {
    let hovered = cx.hovered(r) && !disabled;
    let hv = cx.tween(hover, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
    let (bg, outline, text) = variant_colors(cx, variant);
    let hover_bg = if variant == ButtonVariant::Ghost { cx.sch.main } else { cx.sch.better(bg, 10.0) };
    let bg = crate::color::lerp(bg, hover_bg, hv);
    let t = if disabled { 0.5 } else { 0.0 };
    let corners = Corners::same(cx.r());
    cx.rect(r, corners, cx.col(bg, t));
    cx.stroke(r, corners, 1.0, cx.col(outline, t), StrokeKind::Inside);
    let shown = cx.truncate(title, 14.0, r.width() - cx.px(8.0), true);
    cx.text(r, &shown, 14.0, cx.col(text, t), Align::Center, false, true);
    if let Some(p) = wait {
        cx.animate();
        let bar = Rect::from_min_size(Pos2::new(r.min.x, r.max.y - cx.px(2.0)), Vec2::new(r.width() * p.clamp(0.0, 1.0), cx.px(2.0)));
        cx.rect(bar, Corners::bottom(cx.r()), cx.col(cx.sch.accent, 0.0));
    }
    !disabled && cx.clicked(r)
}

/// Right-aligned, wrapping rows of button widths. Returns `(row index, x offset from the right edge)` per button and the row count.
fn layout_button_rows(widths: &[f32], avail: f32, gap: f32) -> (Vec<(usize, f32)>, usize) {
    let mut out = Vec::with_capacity(widths.len());
    let mut row = 0usize;
    let mut used = 0.0f32;
    let mut rows: Vec<f32> = Vec::new();
    for &w in widths {
        let next = if used == 0.0 { w } else { used + gap + w };
        if used > 0.0 && next > avail {
            rows.push(used);
            row += 1;
            used = w;
        } else {
            used = next;
        }
        out.push((row, used));
    }
    rows.push(used);
    // Convert "right edge of the button within its row" to an offset from the row's right edge.
    for (i, (r, right)) in out.iter_mut().enumerate() {
        let row_w = rows[*r];
        *right = row_w - *right + widths[i];
    }
    (out, rows.len())
}

pub(crate) fn pass_all(m: &mut Model, win: NodeId, cx: &mut Cx, main: Rect) {
    let dialogs = match &m.nodes[win].kind {
        Kind::Window(w) => w.dialogs.clone(),
        _ => return,
    };
    if dialogs.is_empty() {
        return;
    }
    let now = cx.time;
    let s = cx.m.s;
    let outer_blocked = cx.blocked;
    // Only the topmost live dialog takes input.
    let top = dialogs.iter().rev().copied().find(|d| matches!(&m.nodes[*d].kind, Kind::Dialog(dd) if !dd.closing));

    for id in dialogs {
        let Some(Kind::Dialog(d)) = m.nodes.get(id).map(|n| &n.kind) else { continue };
        if d.closing && now >= d.close_at {
            m.destroy_subtree(id);
            continue;
        }
        let (title, description, icon, title_color, desc_color, auto_dismiss, outside_dismiss, closing) =
            (d.title.clone(), d.description.clone(), d.icon.clone(), d.title_color, d.description_color, d.auto_dismiss, d.outside_click_dismiss, d.closing);
        let buttons: Vec<(String, String, ButtonVariant, bool, Option<f64>, f32)> =
            d.buttons.iter().map(|b| (b.key.clone(), b.title.clone(), b.variant, b.disabled, b.wait_until, b.wait_time)).collect();
        let children = m.nodes[id].children.clone();
        let open = {
            let Some(Kind::Dialog(d)) = m.nodes.get(id).map(|n| &n.kind) else { continue };
            cx.anim_value(&d.open)
        };
        let interactive = !outer_blocked && top == Some(id) && !closing && m.open && cx.fade > 0.99;

        // Overlay.
        cx.rect(main, Corners::same(cx.r()), cx.col(cx.sch.dark, 1.0 - 0.5 * open));
        record_overlay(m, Layer::Window, main);

        // Width.
        let widths: Vec<f32> = buttons.iter().map(|b| button_width(cx, &b.1)).collect();
        let width_design = if widths.is_empty() {
            400.0
        } else {
            let required = widths.iter().sum::<f32>() / s + 8.0 * (widths.len() - 1) as f32 + 30.0;
            required.min(main.width() / s * 0.75).max(400.0)
        };
        let w = cx.px(width_design);
        let inner_w = w - cx.px(30.0);
        let gap = cx.px(8.0);

        // Measure.
        let has_icon = icon.as_ref().map(|i| cx.resolve_icon(i, (cx.px(16.0) * cx.m.ppp) as u32).is_some()).unwrap_or(false);
        let title_h = cx.text_size(&title, 18.0, None, true).y.max(cx.px(20.0));
        let desc_h = if description.is_empty() { 0.0 } else { cx.text_size(&description, 14.0, Some(inner_w), true).y };
        let header_h = title_h + if desc_h > 0.0 { cx.px(6.0) + desc_h } else { 0.0 } + cx.px(5.0);
        let content_h = crate::elements::measure_children(m, &children, cx, inner_w, gap);
        let (rows, row_count) = layout_button_rows(&widths, inner_w, gap);
        let buttons_h = if widths.is_empty() { 0.0 } else { cx.px(5.0) + row_count as f32 * cx.px(26.0) + (row_count - 1) as f32 * gap };
        let mut h = cx.px(15.0) + header_h;
        if content_h > 0.0 {
            h += cx.px(10.0) + content_h + cx.px(5.0);
        }
        if !widths.is_empty() {
            h += cx.px(10.0) + cx.px(1.0) + cx.px(10.0) + buttons_h;
        }
        h += cx.px(15.0);

        // Frame (scaled around the center of the main frame).
        let scale = 0.95 + 0.05 * open;
        let frame = Rect::from_center_size(main.center(), Vec2::new(w, h) * scale);
        m.nodes[id].rect = frame;
        if interactive && outside_dismiss && cx.pressed(crate::input::MouseButton::Left) && cx.hovered(main) && !frame.contains(cx.pointer().unwrap_or(Pos2::ZERO)) {
            dismiss(m, id);
        }
        let corners = Corners::same(cx.r());
        cx.rect(frame, corners, cx.col(cx.sch.background, 0.0));

        let prev_blocked = cx.blocked;
        cx.blocked = !interactive;
        cx.with_clip(frame, |cx| {
            let x = frame.min.x + cx.px(15.0);
            let iw = frame.width() - cx.px(30.0);
            let mut y = frame.min.y + cx.px(15.0);

            // Header.
            let tc = title_color.unwrap_or(cx.sch.font_color);
            let mut tx = x;
            if has_icon {
                let ir = Rect::from_center_size(Pos2::new(x + cx.px(8.0), y + title_h / 2.0), Vec2::splat(cx.px(16.0)));
                if let Some(i) = &icon {
                    cx.icon(i, ir, cx.col(tc, 0.0));
                }
                tx += cx.px(22.0);
            }
            let tr = Rect::from_min_size(Pos2::new(tx, y), Vec2::new((x + iw - tx).max(1.0), title_h));
            cx.with_clip(tr, |cx| {
                cx.text(tr, &title, 18.0, cx.col(tc, 0.0), Align::Min, false, true);
            });
            y += title_h;
            if desc_h > 0.0 {
                y += cx.px(6.0);
                let dr = Rect::from_min_size(Pos2::new(x, y), Vec2::new(iw, desc_h));
                let (dc, dt) = match desc_color {
                    Some(c) => (c, 0.0),
                    None => (cx.sch.font_color, 0.2),
                };
                cx.text_top(dr, &description, 14.0, cx.col(dc, dt), Align::Min, true);
                y += desc_h;
            }
            y += cx.px(5.0);

            // Elements.
            if content_h > 0.0 {
                y += cx.px(10.0);
                let used = crate::elements::pass_children(m, &children, cx, x, y, iw, gap, false);
                y += used + cx.px(5.0);
            }

            // Separator + buttons.
            if !buttons.is_empty() {
                y += cx.px(10.0);
                cx.hline(x, x + iw, y, cx.col(cx.sch.outline, 0.0));
                y += cx.px(1.0) + cx.px(10.0) + cx.px(5.0);
                let mut clicked: Option<String> = None;
                for (i, b) in buttons.iter().enumerate() {
                    let (row, from_right) = rows[i];
                    let bw = widths[i];
                    let bx = x + iw - from_right;
                    let by = y + row as f32 * (cx.px(26.0) + gap);
                    let r = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(bw, cx.px(26.0)));
                    let (key, title, variant, mut disabled, wait_until, wait_time) = (&b.0, &b.1, b.2, b.3, b.4, b.5);
                    let mut wait = None;
                    if let Some(until) = wait_until {
                        if now < until {
                            disabled = true;
                            wait = Some(1.0 - ((until - now) as f32 / wait_time.max(1e-3)));
                        } else if let Some(Kind::Dialog(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
                            // Wait elapsed: enable once.
                            if let Some(btn) = d.buttons.iter_mut().find(|bb| bb.key == *key) {
                                btn.wait_until = None;
                                btn.disabled = false;
                                disabled = false;
                            }
                        }
                    }
                    let mut hover = match &m.nodes[id].kind {
                        Kind::Dialog(d) => d.buttons.iter().find(|bb| bb.key == *key).map(|bb| bb.hover).unwrap_or_default(),
                        _ => Anim::default(),
                    };
                    let hit = draw_variant_button(cx, r, title, variant, &mut hover, disabled, wait);
                    if let Kind::Dialog(d) = &mut m.nodes[id].kind {
                        if let Some(btn) = d.buttons.iter_mut().find(|bb| bb.key == *key) {
                            btn.hover = hover;
                            btn.rect = r;
                        }
                    }
                    if hit {
                        clicked = Some(key.clone());
                    }
                }
                if let Some(key) = clicked {
                    m.pending.push(Pending::DialogButton(id, key));
                    if auto_dismiss {
                        dismiss(m, id);
                    }
                }
            }
        });
        cx.blocked = prev_blocked;
        cx.outline(frame, corners, 0.0);
        if closing || open < 1.0 {
            cx.animate();
        }
    }
}
