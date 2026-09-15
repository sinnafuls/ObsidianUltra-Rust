//! Notifications (toasts).

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, Layer};
use crate::handles::info::NotifyInfo;
use crate::handles::Notification;
use crate::node::*;
use crate::tween::{info as tw, Anim};
use crate::types::Side;
use crate::ui::{HistoryEntry, Model, Ui};

impl Ui {
    pub fn notify(&self, info: NotifyInfo) -> Notification {
        let id = self.with(|m| create(m, info));
        Notification { ui: self.clone(), id }
    }
    pub fn notify_text(&self, text: &str, secs: f32) -> Notification {
        self.notify(NotifyInfo { description: text.to_owned(), time: secs, ..Default::default() })
    }
}

fn utc_time_string(secs: u64) -> String {
    let s = secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

pub(crate) fn create(m: &mut Model, info: NotifyInfo) -> NodeId {
    let now = m.time;
    let data = NotifyData {
        title: info.title.clone(),
        title_color: info.title_color,
        description: info.description.clone(),
        description_color: info.description_color,
        time: info.time.max(0.0),
        steps: info.steps,
        step: 0,
        persist: info.persist,
        icon: info.icon.clone(),
        big_icon: info.big_icon,
        icon_color: info.icon_color,
        kind: info.kind,
        created: now,
        slide: Anim::new(0.0),
        y: Anim::new(0.0),
        y_initialized: false,
        destroying: false,
        destroy_at: 0.0,
        height: 0.0,
        width: 0.0,
    };
    let id = m.insert(Kind::Notification(data), None);
    m.notifications.push(id);
    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (title_color, description_color) = type_colors(m, info.kind, info.title.is_some(), info.title_color, info.description_color);
    m.push_history(HistoryEntry {
        title: info.title,
        description: info.description,
        title_color,
        description_color,
        icon: info.icon,
        icon_color: info.icon_color,
        kind: info.kind,
        timestamp,
        time_string: utc_time_string(timestamp),
        copied: None,
        hover: Anim::new(0.0),
    });
    id
}

/// Resolve title/description colors from the notification type (`Notify` color logic).
pub(crate) fn type_colors(m: &Model, kind: Option<crate::types::NotifyType>, has_title: bool, title: Option<epaint::Color32>, desc: Option<epaint::Color32>) -> (Option<epaint::Color32>, Option<epaint::Color32>) {
    let Some(k) = kind else { return (title, desc) };
    let c = m.settings.type_color(k);
    if has_title { (title.or(Some(c)), desc) } else { (title, desc.or(Some(c))) }
}

/// `Notification:Destroy` (starts the slide-out; removed after the tween).
pub(crate) fn destroy(m: &mut Model, id: NodeId) {
    if let Some(Kind::Notification(n)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
        if !n.destroying {
            n.destroying = true;
            n.destroy_at = m.time + crate::tween::info::NOTIFY.0 as f64;
        }
    }
}

/// Per-notification measurements (points).
struct Measure {
    width: f32,
    height: f32,
    /// Big icon shown (24px + 8px gap).
    big: bool,
    /// Title icon shown (15px + 6px gap).
    title_icon: bool,
    title: Option<(Vec2, f32)>,
    desc: Vec2,
    content_h: f32,
    timer: bool,
}

fn measure(cx: &mut Cx, n: &NotifyData) -> Measure {
    let big = n.big_icon.as_ref().map(|i| cx.resolve_icon(i, (cx.px(24.0) * cx.m.ppp) as u32).is_some()).unwrap_or(false);
    let extra = if big { 32.0 } else { 0.0 };
    let title_icon = n.title.is_some() && n.icon.as_ref().map(|i| cx.resolve_icon(i, (cx.px(15.0) * cx.m.ppp) as u32).is_some()).unwrap_or(false);
    let icon_w = if title_icon { 21.0 } else { 0.0 };
    let title = n.title.as_ref().map(|t| {
        let sz = cx.text_size(t, 15.0, Some(cx.px(300.0 - 24.0 - extra - icon_w).max(1.0)), true);
        let h = sz.y.max(if title_icon { cx.px(16.0) } else { 0.0 });
        (sz, h)
    });
    let desc = cx.text_size(&n.description, 14.0, Some(cx.px(300.0 - 24.0 - extra).max(1.0)), true);
    let title_x = title.map(|(sz, _)| sz.x + cx.px(icon_w)).unwrap_or(0.0);
    let width = title_x.max(desc.x) + cx.px(24.0 + extra);
    let text_h = title.map(|(_, h)| h + cx.px(4.0)).unwrap_or(0.0) + desc.y;
    let content_h = if big { text_h.max(cx.px(24.0)) } else { text_h };
    let timer = !n.persist || n.steps.is_some();
    let height = cx.px(16.0) + content_h + if timer { cx.px(4.0 + 7.0) } else { 0.0 };
    Measure { width, height, big, title_icon, title, desc, content_h, timer }
}

pub(crate) fn pass(m: &mut Model, cx: &mut Cx) {
    if m.notifications.is_empty() {
        return;
    }
    let ids = m.notifications.clone();
    let screen = m.screen;
    let left = m.settings.notify_side == Side::Left;
    let inset = cx.px(6.0);
    let col_x = if left { screen.min.x + inset } else { screen.max.x - inset };
    let col_y = screen.min.y + inset;
    let now = cx.time;
    let font_c = cx.sch.font_color;

    cx.with_layer(Layer::Notifications, screen, true, |cx| {
        let mut running = 0.0f32;
        for id in ids {
            let Some(Kind::Notification(n)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { continue };
            if !n.y_initialized {
                // The slide-in (and the timer after it) starts when first shown.
                n.created = now;
            }
            if n.destroying && now >= n.destroy_at {
                m.destroy_subtree(id);
                continue;
            }
            // Auto destroy after the slide-in plus `time`.
            if !n.persist && !n.destroying && now >= n.created + tw::NOTIFY.0 as f64 + n.time as f64 {
                destroy(m, id);
            }
            let Some(Kind::Notification(n)) = m.nodes.get(id).map(|n| &n.kind) else { continue };
            let (title, desc, title_color, desc_color, kind, icon, big_icon, icon_color, created, time, steps, step, destroying, y_init) = (
                n.title.clone(),
                n.description.clone(),
                n.title_color,
                n.description_color,
                n.kind,
                n.icon.clone(),
                n.big_icon.clone(),
                n.icon_color,
                n.created,
                n.time,
                n.steps,
                n.step,
                n.destroying,
                n.y_initialized,
            );
            let ms = measure(cx, n);

            // Vertical position: snap on first placement, tween afterwards; a departing
            // notification keeps its slot while the others close the gap.
            let target_y = running;
            let (y_off, slide) = {
                let Some(Kind::Notification(n)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { continue };
                n.width = ms.width;
                n.height = ms.height;
                let y = if destroying {
                    cx.anim_value(&n.y)
                } else if !y_init {
                    n.y.snap(target_y);
                    n.y_initialized = true;
                    target_y
                } else {
                    cx.tween(&mut n.y, target_y, tw::NOTIFY)
                };
                let mut sl = n.slide;
                let v = cx.tween(&mut sl, if destroying { 0.0 } else { 1.0 }, tw::NOTIFY);
                n.slide = sl;
                (y, v)
            };
            if !destroying {
                running += ms.height + cx.px(8.0);
            }

            let off = (1.0 - slide) * (ms.width + cx.px(8.0));
            let x = if left { col_x - off } else { col_x - ms.width + off };
            let y = col_y + y_off - (1.0 - slide) * cx.px(2.0);
            let card = Rect::from_min_size(Pos2::new(x, y), Vec2::new(ms.width, ms.height));
            m.nodes[id].rect = card;

            // Card.
            let corners = Corners::same(cx.r());
            cx.rect(card, corners, cx.col(cx.sch.main, 0.0));
            cx.outline(card, corners, 0.0);
            let inner = card.shrink(cx.px(8.0));
            let content = Rect::from_min_size(inner.min, Vec2::new(inner.width(), ms.content_h));
            let mut tx = content.min.x;
            if ms.big {
                let ir = Rect::from_center_size(Pos2::new(content.min.x + cx.px(12.0), content.center().y), Vec2::splat(cx.px(24.0)));
                if let Some(bi) = &big_icon {
                    cx.icon(bi, ir, cx.col(icon_color.unwrap_or(cx.sch.accent), 0.0));
                }
                tx += cx.px(32.0);
            }
            let (tc, dc) = type_colors(m, kind, title.is_some(), title_color, desc_color);
            let text_h = ms.title.map(|(_, h)| h + cx.px(4.0)).unwrap_or(0.0) + ms.desc.y;
            let mut ty = content.min.y + (content.height() - text_h) / 2.0;
            if let (Some(t), Some((sz, h))) = (&title, ms.title) {
                let row = Rect::from_min_size(Pos2::new(tx, ty), Vec2::new(content.max.x - tx, h));
                let mut lx = tx;
                if ms.title_icon {
                    let ir = Rect::from_center_size(Pos2::new(tx + cx.px(7.5), row.center().y + cx.px(1.0)), Vec2::splat(cx.px(15.0)));
                    if let Some(ic) = &icon {
                        cx.icon(ic, ir, cx.col(icon_color.unwrap_or(font_c), 0.0));
                    }
                    lx += cx.px(21.0);
                }
                let tr = Rect::from_min_size(Pos2::new(lx, row.min.y), Vec2::new(sz.x.max(1.0), h));
                cx.text(tr, t, 15.0, cx.col(tc.unwrap_or(font_c), 0.0), Align::Min, true, true);
                ty += h + cx.px(4.0);
            }
            let dr = Rect::from_min_size(Pos2::new(tx, ty), Vec2::new(ms.desc.x.max(1.0), ms.desc.y));
            cx.text_top(dr, &desc, 14.0, cx.col(dc.unwrap_or(font_c), 0.0), Align::Min, true);

            // Timer bar.
            if ms.timer {
                let holder_y = content.max.y + cx.px(4.0);
                let bar = Rect::from_min_size(Pos2::new(inner.min.x, holder_y + cx.px(3.0)), Vec2::new(inner.width(), cx.px(2.0)));
                cx.rect(bar, 0.0, cx.col(cx.sch.background, 0.0));
                cx.stroke(bar, 0.0, 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Outside);
                let frac = match steps {
                    Some(total) if total > 0 => step as f32 / total as f32,
                    _ => {
                        let elapsed = (now - created - tw::NOTIFY.0 as f64) as f32;
                        if elapsed <= 0.0 {
                            1.0
                        } else if time <= 0.0 {
                            0.0
                        } else {
                            let f = 1.0 - (elapsed / time).clamp(0.0, 1.0);
                            if f > 0.0 {
                                cx.animate();
                            }
                            f
                        }
                    }
                };
                if frac > 0.0 {
                    let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac.clamp(0.0, 1.0), bar.height()));
                    cx.rect(fill, 0.0, cx.col(cx.sch.accent, 0.0));
                }
            }
            if !destroying && now < created + tw::NOTIFY.0 as f64 + time as f64 {
                cx.animate();
            }
        }
    });
}
