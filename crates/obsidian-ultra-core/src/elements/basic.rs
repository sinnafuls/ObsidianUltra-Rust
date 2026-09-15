//! Divider, Label, Button (+ sub-button).

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::frame::Cx;
use crate::handles::info::{ButtonInfo, DividerInfo, LabelInfo};
use crate::node::*;
use crate::tween::info as tw;
use crate::ui::{Model, Pending};

// ----- constructors --------------------------------------------------------------------------

pub(crate) fn create_divider(m: &mut Model, parent: NodeId, info: DividerInfo) -> NodeId {
    let id = m.insert(Kind::Divider(DividerData { text: info.text, margin_top: info.margin_top.max(0.0), margin_bottom: info.margin_bottom.max(0.0) }), Some(parent));
    m.add_child(parent, id);
    id
}

pub(crate) fn create_label(m: &mut Model, parent: NodeId, idx: &str, info: LabelInfo) -> NodeId {
    let id = m.insert(Kind::Label(LabelData { text: info.text, does_wrap: info.does_wrap, size: info.size.max(1.0), addons: Vec::new(), height: 0.0 }), Some(parent));
    m.nodes[id].visible = info.visible;
    m.nodes[id].idx = idx.to_owned();
    m.add_child(parent, id);
    m.labels.push(id);
    id
}

pub(crate) fn create_button(m: &mut Model, parent: NodeId, idx: &str, info: ButtonInfo, is_sub: bool) -> NodeId {
    let data = ButtonData {
        text: info.text,
        func: info.callback.into_iter().collect(),
        double_click: info.double_click,
        tips: Tips { tooltip: info.tooltip, disabled_tooltip: info.disabled_tooltip },
        risky: info.risky,
        disabled: info.disabled,
        is_sub,
        sub: None,
        confirm_until: None,
        addons: Vec::new(),
    };
    let id = m.insert(Kind::Button(data), Some(parent));
    m.nodes[id].visible = info.visible;
    m.nodes[id].idx = idx.to_owned();
    if is_sub {
        if let Kind::Button(b) = &mut m.nodes[parent].kind {
            if let Some(old) = b.sub.replace(id) {
                m.destroy_subtree(old);
            }
        }
    } else {
        m.add_child(parent, id);
    }
    m.buttons.push(id);
    id
}

// ----- divider -------------------------------------------------------------------------------

pub(crate) fn divider_height(m: &Model, id: NodeId, cx: &Cx) -> f32 {
    let Kind::Divider(d) = &m.nodes[id].kind else { return 0.0 };
    cx.px(6.0 + d.margin_top + d.margin_bottom)
}

pub(crate) fn pass_divider(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let Kind::Divider(d) = &m.nodes[id].kind else { return 0.0 };
    let (text, mt, mb) = (d.text.clone(), d.margin_top, d.margin_bottom);
    let h = cx.px(6.0 + mt + mb);
    let inner = Rect::from_min_max(Pos2::new(x, y + cx.px(mt)), Pos2::new(x + w, y + h - cx.px(mb)));
    let line_h = cx.px(2.0);
    let cy = inner.center().y;
    let main = cx.col(cx.sch.main, 0.0);
    let outline = cx.col(cx.sch.outline, 0.0);
    let draw_line = |cx: &mut Cx, x0: f32, x1: f32| {
        if x1 <= x0 {
            return;
        }
        let r = Rect::from_min_max(Pos2::new(x0, cy - line_h / 2.0), Pos2::new(x1, cy + line_h / 2.0));
        cx.rect(r, 0.0, main);
        cx.stroke(r, 0.0, 1.0, outline, StrokeKind::Outside);
    };
    match text.filter(|t| !t.is_empty()) {
        None => draw_line(cx, inner.min.x, inner.max.x),
        Some(t) => {
            let tw_ = cx.text_size(&t, 14.0, None, true).x;
            let gap = (tw_ / 2.0).floor() + cx.px(10.0);
            draw_line(cx, inner.min.x, inner.center().x - gap);
            draw_line(cx, inner.center().x + gap, inner.max.x);
            cx.text(inner, &t, 14.0, cx.col(cx.sch.font_color, 0.5), Align::Center, false, true);
        }
    }
    h
}

// ----- label ---------------------------------------------------------------------------------

pub(crate) fn label_height(m: &mut Model, id: NodeId, cx: &mut Cx, w: f32) -> f32 {
    let Kind::Label(l) = &m.nodes[id].kind else { return 0.0 };
    if l.does_wrap {
        let (text, size) = (l.text.clone(), l.size);
        cx.text_size(&text, size, Some(w.max(1.0)), true).y + cx.px(4.0)
    } else {
        cx.px(18.0)
    }
}

pub(crate) fn pass_label(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let Kind::Label(l) = &m.nodes[id].kind else { return 0.0 };
    let (text, wrap, size, addons) = (l.text.clone(), l.does_wrap, l.size, l.addons.clone());
    let h = label_height(m, id, cx, w);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    let color = cx.col(cx.sch.font_color, 0.0);
    let align = if cx.key_tab { Align::Center } else { Align::Min };
    if wrap {
        cx.text_top(Rect::from_min_size(Pos2::new(x, y + cx.px(2.0)), Vec2::new(w, h)), &text, size, color, align, true);
    } else {
        // Addons stack from the right edge.
        let mut right = r.max.x;
        for a in addons.iter().rev() {
            let used = pass_addon(m, *a, cx, right, r.center().y);
            if used > 0.0 {
                right -= used + cx.px(6.0);
            }
        }
        let tr = Rect::from_min_max(r.min, Pos2::new(right, r.max.y));
        cx.with_clip(tr, |cx| {
            cx.text(tr, &text, size, color, align, false, true);
        });
    }
    if let Kind::Label(l) = &mut m.nodes[id].kind {
        l.height = h;
    }
    h
}

/// Draw an addon (key picker / color picker) right-aligned at `right_x`, vertically centered at `cy`.
/// Returns the width used.
pub(crate) fn pass_addon(m: &mut Model, addon: NodeId, cx: &mut Cx, right_x: f32, cy: f32) -> f32 {
    if !m.alive(addon) {
        return 0.0;
    }
    match &m.nodes[addon].kind {
        Kind::KeyPicker(_) => super::keypicker::pass_addon(m, addon, cx, right_x, cy),
        Kind::ColorPicker(_) => super::colorpicker::pass_addon(m, addon, cx, right_x, cy),
        _ => 0.0,
    }
}

// ----- button --------------------------------------------------------------------------------

pub(crate) fn button_height(_m: &Model, _id: NodeId, cx: &Cx) -> f32 {
    cx.px(21.0)
}

pub(crate) fn pass_button(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let h = cx.px(21.0);
    let (sub, addons) = match &m.nodes[id].kind { Kind::Button(b) => (b.sub.filter(|s| m.alive(*s)), b.addons.clone()), _ => return 0.0 };
    let gap = cx.px(9.0);
    // A Press-mode key picker sits at the right edge (18 wide) of the row.
    let mut right = x + w;
    for a in addons.iter().rev() {
        if !m.alive(*a) {
            continue;
        }
        let used = super::keypicker::pass_addon(m, *a, cx, right, y + h / 2.0);
        if used > 0.0 {
            right -= used + gap;
        }
    }
    let row_w = right - x;
    let sub_shown = sub.map(|s| m.nodes[s].shown()).unwrap_or(false);
    let main_shown = m.nodes[id].shown();
    match (main_shown, sub_shown) {
        (true, true) => {
            let bw = (row_w - gap) / 2.0;
            draw_button(m, id, cx, Rect::from_min_size(Pos2::new(x, y), Vec2::new(bw, h)));
            draw_button(m, sub.unwrap(), cx, Rect::from_min_size(Pos2::new(x + bw + gap, y), Vec2::new(bw, h)));
        }
        (true, false) => draw_button(m, id, cx, Rect::from_min_size(Pos2::new(x, y), Vec2::new(row_w, h))),
        (false, true) => draw_button(m, sub.unwrap(), cx, Rect::from_min_size(Pos2::new(x, y), Vec2::new(row_w, h))),
        (false, false) => return 0.0,
    }
    h
}

fn draw_button(m: &mut Model, id: NodeId, cx: &mut Cx, r: Rect) {
    let Kind::Button(b) = &m.nodes[id].kind else { return };
    let (text, disabled, risky, double, confirm_until, tips) = (b.text.clone(), b.disabled, b.risky, b.double_click, b.confirm_until, b.tips.clone());
    m.nodes[id].rect = r;
    let hovered = cx.hovered(r) && !disabled;
    let hv = {
        let n = &mut m.nodes[id];
        let mut a = n.hover;
        let v = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
        n.hover = a;
        v
    };
    let confirming = confirm_until.map(|t| cx.time < t).unwrap_or(false);
    if confirm_until.is_some() && !confirming {
        if let Kind::Button(b) = &mut m.nodes[id].kind {
            b.confirm_until = None;
        }
    }
    let pill = cx.pill(r);
    let bg = if disabled { cx.sch.background } else { cx.sch.main };
    cx.rect(r, pill, cx.col(bg, 0.0));
    cx.stroke(r, pill, 1.0, cx.col(cx.sch.outline, if disabled { 0.5 } else { 0.0 }), StrokeKind::Inside);
    let t = super::text_transparency(disabled, hv, false);
    let (label, color) = if confirming {
        ("Are you sure?".to_owned(), cx.sch.accent)
    } else {
        (text, if risky { cx.sch.red } else { cx.sch.font_color })
    };
    let color = cx.col(color, t);
    let shown = cx.truncate(&label, 14.0, r.width() - cx.px(8.0), true);
    cx.text(r, &shown, 14.0, color, Align::Center, false, true);
    if let Some(tt) = tips.text(disabled) {
        if cx.hovered(r) {
            cx.tooltip(id, tt);
        }
    }
    if confirming {
        cx.animate();
    }
    if !disabled && cx.clicked(r) {
        if double {
            if confirming {
                if let Kind::Button(b) = &mut m.nodes[id].kind {
                    b.confirm_until = None;
                }
                m.pending.push(Pending::Unit(id));
            } else if let Kind::Button(b) = &mut m.nodes[id].kind {
                b.confirm_until = Some(cx.time + 0.5);
            }
        } else {
            m.pending.push(Pending::Unit(id));
        }
    }
}
