//! Notification history panel.

use epaint::emath::Align;
use epaint::{Pos2, Rect, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, Layer};
use crate::input::MouseButton;
use crate::node::{cap, Capture};
use crate::tween::{info as tw, Ease};
use crate::types::IconRef;
use crate::ui::{HistoryEntry, Model, Ui};
use crate::window::record_overlay;

impl Ui {
    pub fn notification_history(&self) -> Vec<HistoryEntry> {
        self.with(|m| m.history.clone())
    }

    pub fn clear_notification_history(&self) {
        self.with(|m| m.history.clear());
    }

    pub fn set_notification_history_visible(&self, visible: bool) {
        self.with(|m| set_visible(m, visible));
    }

    pub fn toggle_notification_history(&self) {
        self.with(|m| set_visible(m, !m.history_open));
    }

    pub fn is_notification_history_open(&self) -> bool {
        self.with(|m| m.history_open)
    }
}
pub(crate) fn set_visible(m: &mut Model, visible: bool) {
    if m.history_open == visible {
        return;
    }
    m.history_open = visible;
    if visible {
        m.history_unread = 0;
        m.history_visible = true;
    }
}

const WIDTH: f32 = 288.0;
const HEIGHT: f32 = 328.0;
const SLIDE: f32 = 22.0;
const TITLE_H: f32 = 34.0;

/// Panel top-left under the bell, kept 6px inside the screen.
fn rest_position(m: &Model, cx: &Cx) -> Pos2 {
    let screen = m.screen;
    let size = Vec2::new(cx.px(WIDTH), cx.px(HEIGHT));
    let margin = cx.px(6.0);
    let p = Pos2::new(m.history_rest.x - size.x, m.history_rest.y + cx.px(10.0));
    let max_x = (screen.max.x - margin - size.x).max(screen.min.x + margin);
    let max_y = (screen.max.y - margin - size.y).max(screen.min.y + margin);
    Pos2::new(p.x.clamp(screen.min.x + margin, max_x), p.y.clamp(screen.min.y + margin, max_y))
}

/// Per-card measurements (points).
struct CardMeasure {
    height: f32,
    title: Option<Vec2>,
    desc: Vec2,
}

fn measure_card(cx: &mut Cx, e: &HistoryEntry, w: f32) -> CardMeasure {
    let wrap = (w - cx.px(32.0)).max(1.0);
    let title = e.title.as_ref().map(|t| cx.text_size(t, 15.0, Some(wrap), true));
    let desc = cx.text_size(&e.description, 14.0, Some(wrap), true);
    let mut h = cx.px(6.0 + 14.0 + 2.0) + desc.y + cx.px(6.0);
    if let Some(t) = title {
        h += t.y + cx.px(2.0);
    }
    CardMeasure { height: h, title, desc }
}

pub(crate) fn pass(m: &mut Model, cx: &mut Cx, blocked: bool) {
    let now = cx.time;

    // Open / close transitions.
    let mut just_opened = false;
    if m.history_open && m.history_anim.target() < 0.5 {
        m.history_pos = rest_position(m, cx);
        m.history_anim.play(0.0, 1.0, now, tw::HISTORY_OPEN.0, tw::HISTORY_OPEN.1);
        m.history_scroll.offset = 0.0;
        just_opened = true;
    } else if !m.history_open && m.history_visible && m.history_anim.target() > 0.5 {
        m.history_anim.set(0.0, now, tw::HISTORY_CLOSE.0, tw::HISTORY_CLOSE.1);
        m.history_hide_at = now + tw::HISTORY_CLOSE.0 as f64;
    }
    if !m.history_visible {
        return;
    }
    if !m.history_open && now >= m.history_hide_at {
        m.history_visible = false;
        return;
    }
    let alpha = cx.anim_value(&m.history_anim);

    let screen = m.screen;
    let pointer = cx.pointer();
    let size = Vec2::new(cx.px(WIDTH), cx.px(HEIGHT));
    let capture_node = m.window.unwrap_or_default();

    // Title-bar drag.
    let panel_at = |pos: Pos2| Rect::from_min_size(Pos2::new(pos.x, pos.y - cx.px(SLIDE) * (1.0 - alpha)), size);
    if let Some(c) = m.capture {
        if c.sub == cap::HISTORY_DRAG {
            if let Some(p) = pointer {
                let d = p - c.start;
                m.history_pos = Pos2::new(c.data[0] + d.x, c.data[1] + d.y);
            }
            if !cx.down(MouseButton::Left) {
                m.capture = None;
            }
        }
    }
    let panel = panel_at(m.history_pos);

    // Click outside (not on the panel, not on the bell) closes.
    if m.history_open && !just_opened && cx.pressed(MouseButton::Left) {
        if let Some(p) = pointer {
            let bell = Rect::from_min_size(m.history_rest - Vec2::splat(cx.px(24.0)), Vec2::splat(cx.px(24.0))).expand(cx.px(4.0));
            if !panel.contains(p) && !bell.contains(p) {
                set_visible(m, false);
            }
        }
    }

    let prev_fade = cx.fade;
    cx.fade = alpha;
    let interactive = m.history_open && alpha > 0.99;
    cx.with_layer(Layer::Floats, screen, blocked || !interactive, |cx| {
        let corners = Corners::same(cx.r());
        cx.rect(panel, corners, cx.col(cx.sch.background, 0.0));

        // Title row (drag handle) + close button.
        let title_row = Rect::from_min_size(panel.min, Vec2::new(panel.width(), cx.px(TITLE_H)));
        let close = Rect::from_center_size(Pos2::new(panel.max.x - cx.px(8.0 + 10.0), panel.min.y + cx.px(17.0)), Vec2::splat(cx.px(20.0)));
        let font_c = cx.col(cx.sch.font_color, 0.0);
        let title_rect = Rect::from_min_max(Pos2::new(title_row.min.x + cx.px(12.0), title_row.min.y), Pos2::new(title_row.max.x - cx.px(36.0), title_row.max.y));
        cx.with_clip(title_rect, |cx| {
            cx.text(title_rect, "Notification History", 15.0, font_c, Align::Min, false, false);
        });
        cx.hline(panel.min.x, panel.max.x, panel.min.y + cx.px(TITLE_H), cx.col(cx.sch.outline, 0.0));
        let close_hovered = cx.hovered(close);
        let close_tint = cx.col(cx.sch.font_color, if close_hovered { 0.0 } else { 0.35 });
        let close_icon = Rect::from_center_size(close.center(), Vec2::splat(cx.px(14.0)));
        if !cx.icon(&IconRef::Lucide("x".into()), close_icon, close_tint) {
            cx.text(close, "X", 14.0, close_tint, Align::Center, false, false);
        }
        if cx.clicked(close) {
            set_visible(m, false);
        } else if m.capture.is_none() && cx.clicked(title_row) {
            if let Some(p) = pointer {
                let pos = m.history_pos;
                m.capture = Some(Capture { node: capture_node, sub: cap::HISTORY_DRAG, start: p, data: [pos.x, pos.y, 0.0, 0.0], since: now });
            }
        }

        // Scroller.
        let scroller = Rect::from_min_max(Pos2::new(panel.min.x, panel.min.y + cx.px(TITLE_H + 1.0)), panel.max);
        let pad = cx.px(8.0);
        let gap = cx.px(6.0);
        let card_w = scroller.width() - pad * 2.0;
        let entries: Vec<(usize, HistoryEntry)> = m.history.iter().cloned().enumerate().collect();
        let measures: Vec<CardMeasure> = entries.iter().map(|(_, e)| measure_card(cx, e, card_w)).collect();
        let content_h = if entries.is_empty() {
            cx.px(24.0)
        } else {
            measures.iter().map(|c| c.height).sum::<f32>() + gap * (entries.len() - 1) as f32
        } + pad * 2.0;
        let scroll = &mut m.history_scroll;
        scroll.view = scroller.height();
        scroll.content = content_h;
        let wheel = cx.wheel_over(scroller);
        if wheel != 0.0 {
            scroll.wheel(wheel);
        }
        scroll.clamp();
        let offset = scroll.offset;
        let scrollable = scroll.scrollable();
        let thumb = scroll.thumb(Rect::from_min_max(Pos2::new(scroller.max.x - cx.px(4.0), scroller.min.y), scroller.max));

        cx.with_clip(scroller, |cx| {
            let mut y = scroller.min.y + pad - offset;
            let x = scroller.min.x + pad;
            if entries.is_empty() {
                let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(card_w, cx.px(24.0)));
                cx.text(r, "No notifications yet.", 14.0, cx.col(cx.sch.font_color, 0.4), Align::Min, false, false);
            }
            let success = m.settings.type_color(crate::types::NotifyType::Success);
            let error = m.settings.type_color(crate::types::NotifyType::Error);
            for ((index, e), ms) in entries.iter().zip(measures.iter()) {
                let card = Rect::from_min_size(Pos2::new(x, y), Vec2::new(card_w, ms.height));
                y += ms.height + gap;
                if card.max.y < scroller.min.y || card.min.y > scroller.max.y {
                    continue;
                }
                let hovered = cx.hovered(card);
                let hv = {
                    let mut a = e.hover;
                    let v = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
                    if let Some(entry) = m.history.get_mut(*index) {
                        entry.hover = a;
                    }
                    v
                };
                let cc = Corners::same(cx.r());
                cx.rect(card, cc, cx.col(cx.sch.main, 0.0));
                cx.outline(card, cc, 0.0);
                if hovered {
                    if let Some(w) = m.window {
                        cx.tooltip(w, "Click to copy");
                    }
                }

                // Content.
                let cx0 = card.min.x + cx.px(8.0);
                let cw = card_w - cx.px(32.0);
                let mut cy = card.min.y + cx.px(6.0);
                let header = Rect::from_min_size(Pos2::new(cx0, cy), Vec2::new(cw, cx.px(14.0)));
                let time_text = format!("[{}]", e.time_string);
                let tw_ = cx.text(header, &time_text, 12.0, cx.col(cx.sch.accent, 0.0), Align::Min, false, false).x;
                if let Some(k) = e.kind {
                    let kr = Rect::from_min_max(Pos2::new(cx0 + tw_ + cx.px(6.0), header.min.y), header.max);
                    cx.text(kr, k.label(), 12.0, cx.col(m.settings.type_color(k), 0.0), Align::Min, false, false);
                }
                cy += cx.px(14.0 + 2.0);
                if let (Some(t), Some(ts)) = (&e.title, ms.title) {
                    let tr = Rect::from_min_size(Pos2::new(cx0, cy), Vec2::new(cw, ts.y));
                    cx.text_top(tr, t, 15.0, cx.col(e.title_color.unwrap_or(cx.sch.font_color), 0.0), Align::Min, true);
                    cy += ts.y + cx.px(2.0);
                }
                let dr = Rect::from_min_size(Pos2::new(cx0, cy), Vec2::new(cw, ms.desc.y));
                cx.text_top(dr, &e.description, 14.0, cx.col(e.description_color.unwrap_or(cx.sch.font_color), 0.0), Align::Min, true);

                // Copy icon + feedback.
                let feedback = e.copied.filter(|(until, _)| now < *until);
                let icon_center = Pos2::new(card.max.x - cx.px(7.0 + 6.5), card.min.y + cx.px(7.0 + 6.5));
                match feedback {
                    Some((until, ok)) => {
                        cx.animate();
                        let elapsed = (0.9 - (until - now)) as f32;
                        let pop = crate::tween::ease(Ease::BackOut, elapsed / tw::BADGE_POP.0);
                        let sz = cx.px(9.0 + 4.0 * pop);
                        let color = if ok { success } else { error };
                        let ir = Rect::from_center_size(icon_center, Vec2::splat(sz));
                        let icon = if ok { "clipboard-check" } else { "copy" };
                        if !cx.icon(&IconRef::Lucide(icon.into()), ir, cx.col(color, 0.0)) {
                            cx.icon(&IconRef::Lucide("check".into()), ir, cx.col(color, 0.0));
                        }
                        let rise = crate::tween::ease(Ease::BackOut, elapsed / 0.22);
                        let fade_t = crate::tween::ease(Ease::QuadIn, elapsed / 0.9);
                        let lr = Rect::from_min_size(Pos2::new(card.max.x - cx.px(24.0 + 50.0), card.min.y + cx.px(9.0 - 4.0 * rise)), Vec2::new(cx.px(50.0), cx.px(14.0)));
                        cx.text(lr, if ok { "Copied!" } else { "No clipboard" }, 12.0, cx.col(color, fade_t), Align::Max, false, false);
                    }
                    None => {
                        let ir = Rect::from_center_size(icon_center, Vec2::splat(cx.px(13.0)));
                        cx.icon(&IconRef::Lucide("copy".into()), ir, cx.col(cx.sch.font_color, 0.55 - 0.45 * hv));
                    }
                }
                if cx.clicked(card) {
                    let mut lines = vec![time_text.clone()];
                    if let Some(t) = &e.title {
                        lines.push(t.clone());
                    }
                    lines.push(e.description.clone());
                    cx.out.copy_to_clipboard.push(lines.join("\n"));
                    if let Some(entry) = m.history.get_mut(*index) {
                        entry.copied = Some((now + 0.9, true));
                    }
                }
            }
        });
        if scrollable {
            cx.rect(thumb, cx.pill(thumb), cx.col(cx.sch.accent, 0.0));
        }
        cx.outline(panel, corners, 0.0);
    });
    cx.fade = prev_fade;
    record_overlay(m, Layer::Floats, panel);
}
