//! Shared tooltip label (`TooltipLabel` / `ShowTooltip` / `HideTooltip`).

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, Layer};
use crate::node::Kind;
use crate::tween::info as tw;
use crate::ui::Model;

/// Rect of the currently open context menu (previous frame), if any.
fn current_menu_rect(m: &Model) -> Option<Rect> {
    let (id, sub) = m.current_menu?;
    let n = m.nodes.get(id)?;
    let state = match &n.kind {
        Kind::Dropdown(d) => d.menu,
        Kind::Priority(p) => p.menu,
        Kind::KeyPicker(k) => k.menu,
        Kind::ColorPicker(c) => {
            if sub == 0 {
                c.menu
            } else {
                c.ctx_menu
            }
        }
        _ => return None,
    };
    state.open.then_some(state.rect)
}

pub(crate) fn pass(m: &mut Model, cx: &mut Cx) {
    let mut req = cx.tooltip.take();
    // `DoHover` bails while a dialog is active or the pointer is over the open menu.
    if m.active_dialog.is_some() {
        req = None;
    } else if let (Some(p), Some(r)) = (cx.pointer(), current_menu_rect(m)) {
        if r.contains(p) {
            req = None;
        }
    }

    let time = cx.time;
    match req {
        Some(r) => {
            let restart = !m.tooltip.visible || m.tooltip.node != Some(r.node);
            m.tooltip.node = Some(r.node);
            m.tooltip.text = r.text;
            m.tooltip.visible = true;
            if restart {
                m.tooltip.anim.play(0.0, 1.0, time, tw::TOOLTIP_SHOW.0, tw::TOOLTIP_SHOW.1);
            }
        }
        None => {
            // `visible` may also have been cleared externally (window closed); `set` is a
            // no-op once the target is already 0.
            if m.tooltip.visible {
                m.tooltip.hide_at = time;
            }
            m.tooltip.visible = false;
            m.tooltip.anim.set(0.0, time, tw::TOOLTIP_HIDE.0, tw::TOOLTIP_HIDE.1);
        }
    }

    let v = cx.anim_value(&m.tooltip.anim);
    if v <= 0.001 {
        if !m.tooltip.visible {
            m.tooltip.node = None;
        }
        return;
    }
    let Some(p) = cx.pointer() else { return };
    let (dx, dy) = if m.settings.show_custom_cursor { (8.0, 8.0) } else { (14.0, 12.0) };
    let pos = Pos2::new(p.x + cx.px(dx), p.y + cx.px(dy));
    let text = m.tooltip.text.clone();
    let screen = cx.fi.screen;

    cx.with_layer(Layer::Tooltip, screen, false, |cx| {
        let prev_fade = cx.fade;
        cx.fade = prev_fade * v;
        // Pop scale 0.85 -> 1 around the top-left corner.
        let scale = 0.85 + 0.15 * v;
        let font = 14.0 * scale;
        let pad_x = cx.px(4.0) * scale;
        let pad_y = cx.px(2.0) * scale;
        let wrap = (screen.max.x - pos.x - cx.px(8.0)) * scale;
        let sz = cx.text_size(&text, font, Some(wrap.max(cx.px(20.0))), true);
        let r = Rect::from_min_size(pos, Vec2::new(sz.x + pad_x * 2.0, sz.y + pad_y * 2.0));
        let corners = Corners::same(cx.r2() * scale);
        cx.rect(r, corners, cx.col(cx.sch.background, 0.0));
        cx.stroke(r, corners, 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Inside);
        let tr = Rect::from_min_size(Pos2::new(pos.x + pad_x, pos.y + pad_y), sz);
        cx.text_top(tr, &text, font, cx.col(cx.sch.font_color, 0.0), Align::Min, true);
        cx.fade = prev_fade;
    });
}
