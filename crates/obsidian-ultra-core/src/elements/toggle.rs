//! Toggle (switch) and Checkbox.

use epaint::emath::Align;
use epaint::{Color32, Pos2, Rect, StrokeKind, Vec2};

use crate::color;
use crate::draw::{self, Corners};
use crate::frame::Cx;
use crate::handles::info::ToggleInfo;
use crate::node::*;
use crate::tween::{info as tw, Anim};
use crate::types::*;
use crate::ui::{Model, Pending};

const SWITCH_W: f32 = 38.0;
const SWITCH_H: f32 = 20.0;
const CHECK_H: f32 = 18.0;
const OFF_GRADIENT: (Color32, Color32) = (Color32::from_rgb(0x50, 0x50, 0x50), Color32::from_rgb(0x8A, 0x8A, 0x8A));
const ON_GRADIENT: (Color32, Color32) = (Color32::from_rgb(0xCD, 0xCD, 0xCD), Color32::WHITE);

pub(crate) fn create(m: &mut Model, parent: NodeId, idx: &str, info: ToggleInfo, variant: ToggleVariant) -> NodeId {
    let data = ToggleData {
        text: info.text,
        value: info.default,
        variant,
        listeners: info.callback.into_iter().collect(),
        tips: Tips { tooltip: info.tooltip, disabled_tooltip: info.disabled_tooltip },
        risky: info.risky,
        disabled: info.disabled,
        addons: Vec::new(),
        ball: Anim::new(if info.default { 1.0 } else { 0.0 }),
        label: Anim::new(if info.default { 1.0 } else { 0.0 }),
        default: info.default,
    };
    let id = m.insert(Kind::Toggle(data), Some(parent));
    m.nodes[id].visible = info.visible;
    m.add_child(parent, id);
    m.register_toggle(idx, id);
    id
}
pub(crate) fn set_value(m: &mut Model, id: NodeId, value: bool) {
    let Some(Kind::Toggle(t)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    if t.disabled {
        return;
    }
    t.value = value;
    let addons = t.addons.clone();
    // Sync key pickers.
    let mut picking = false;
    for a in addons {
        if let Some(Kind::KeyPicker(k)) = m.nodes.get_mut(a).map(|n| &mut n.kind) {
            if k.sync_toggle_state {
                k.toggled = value;
            }
            if k.picking.active {
                picking = true;
            }
        }
    }
    m.update_dependency_boxes();
    if !picking {
        m.pending.push(Pending::Bool(id, value));
    }
}

pub(crate) fn height(m: &Model, id: NodeId, cx: &Cx) -> f32 {
    match &m.nodes[id].kind {
        Kind::Toggle(t) if t.variant == ToggleVariant::Checkbox => cx.px(CHECK_H),
        _ => cx.px(SWITCH_H),
    }
}

pub(crate) fn pass(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let Kind::Toggle(t) = &m.nodes[id].kind else { return 0.0 };
    let (text, value, variant, disabled, risky, tips, addons) = (t.text.clone(), t.value, t.variant, t.disabled, t.risky, t.tips.clone(), t.addons.clone());
    let h = height(m, id, cx);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    m.nodes[id].rect = r;
    let hovered = cx.hovered(r) && !disabled;
    let label_v = {
        let Kind::Toggle(t) = &mut m.nodes[id].kind else { return 0.0 };
        let mut a = t.label;
        let v = cx.tween(&mut a, if value { 1.0 } else { 0.0 }, tw::HOVER);
        t.label = a;
        v
    };
    let text_t = if disabled { 0.8 } else { 0.4 * (1.0 - label_v) };
    let label_color = cx.col(if risky { cx.sch.red } else { cx.sch.font_color }, text_t);

    // Addons stack from the label's right edge.
    let control_w = if variant == ToggleVariant::Checkbox { 0.0 } else { cx.px(SWITCH_W + 10.0) };
    let mut right = r.max.x - control_w;
    let label_left = if variant == ToggleVariant::Checkbox { r.min.x + cx.px(26.0) } else { r.min.x };
    for a in addons.iter().rev() {
        let used = super::basic::pass_addon(m, *a, cx, right, r.center().y);
        if used > 0.0 {
            right -= used + cx.px(6.0);
        }
    }
    let lr = Rect::from_min_max(Pos2::new(label_left, r.min.y), Pos2::new(right, r.max.y));
    cx.with_clip(lr, |cx| {
        cx.text(lr, &text, 14.0, label_color, Align::Min, false, true);
    });

    match variant {
        ToggleVariant::Checkbox => {
            let br = Rect::from_min_size(r.min, Vec2::splat(h));
            let bg = if disabled { cx.sch.background } else { cx.sch.main };
            cx.rect(br, cx.r2(), cx.col(bg, 0.0));
            cx.stroke(br, cx.r2(), 1.0, cx.col(cx.sch.outline, if disabled { 0.5 } else { 0.0 }), StrokeKind::Inside);
            let check_t = if disabled { if value { 0.8 } else { 1.0 } } else { 1.0 - label_v };
            if check_t < 0.999 {
                cx.icon(&IconRef::Lucide("check".into()), br.shrink(cx.px(2.0)), cx.col(cx.sch.font_color, check_t));
            }
        }
        ToggleVariant::Switch => {
            let track = Rect::from_min_size(Pos2::new(r.max.x - cx.px(SWITCH_W), r.center().y - cx.px(SWITCH_H) / 2.0), cx.m.v(SWITCH_W, SWITCH_H));
            let pill = cx.pill(track);
            let ball_v = {
                let Kind::Toggle(t) = &mut m.nodes[id].kind else { return 0.0 };
                let mut a = t.ball;
                let v = cx.tween(&mut a, if value { 1.0 } else { 0.0 }, tw::SWITCH_BALL);
                t.ball = a;
                v
            };
            let base = if value { cx.sch.accent } else { cx.sch.font_color };
            let (g0, g1) = if value { ON_GRADIENT } else { OFF_GRADIENT };
            let c0 = color::multiply(base, g0);
            let c1 = color::multiply(base, g1);
            let track_t = if disabled { 0.6 } else { 0.0 };
            let shape = draw::gradient_rounded_rect(track, pill, cx.col(c0, track_t), cx.col(c1, track_t), true);
            cx.push(shape);
            cx.stroke(track, pill, 1.0, cx.col(cx.sch.outline, if value { 1.0 } else { 0.8 }), StrokeKind::Inside);
            let radius = cx.px((SWITCH_H - 4.0) / 2.0);
            let cxp = track.min.x + radius + cx.px(2.0) + (track.width() - 2.0 * radius - cx.px(4.0)) * ball_v;
            let center = Pos2::new(cxp, track.center().y);
            if !disabled {
                cx.push(epaint::Shape::circle_filled(center + Vec2::new(0.0, cx.px(1.0)), radius, cx.col(cx.sch.dark, 0.6)));
            }
            cx.push(epaint::Shape::circle_filled(center, radius, cx.col(cx.sch.font_color, 0.0)));
        }
    }
    let on_addon = addons.iter().any(|a| m.nodes.get(*a).map(|n| cx.hovered(n.rect)).unwrap_or(false));
    if let Some(tt) = tips.text(disabled) {
        if cx.hovered(r) && !on_addon {
            cx.tooltip(id, tt);
        }
    }
    if hovered && !on_addon && cx.clicked(r) {
        set_value(m, id, !value);
    }
    let _ = Corners::ZERO;
    h
}
