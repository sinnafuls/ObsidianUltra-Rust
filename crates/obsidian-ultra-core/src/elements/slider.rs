//! Slider (regular and compact) with drag and right-click numeric entry.

use epaint::emath::Align;
use epaint::{Color32, Pos2, Rect, StrokeKind, Vec2};

use crate::draw::{self, Corners};
use crate::frame::{Cx, FocusRequest, TextFieldRequest};
use crate::handles::info::SliderInfo;
use crate::input::MouseButton;
use crate::node::*;
use crate::tween::{info as tw, Anim};
use crate::ui::{Model, Pending};

const BAR_H: f32 = 16.0;
const BALL: f32 = 18.0;
const BALL_ACTIVE: f32 = 24.0;
const BALL_MARGIN: f32 = 4.0;
const TRACK_GRADIENT: (Color32, Color32) = (Color32::from_rgb(0x8A, 0x8A, 0x8A), Color32::from_rgb(0x40, 0x40, 0x40));

pub(crate) fn create(m: &mut Model, parent: NodeId, idx: &str, info: SliderInfo) -> NodeId {
    let data = SliderData {
        text: info.text,
        value: info.default,
        min: info.min,
        max: info.max,
        rounding: info.rounding,
        prefix: info.prefix,
        suffix: info.suffix,
        compact: info.compact,
        hide_max: info.hide_max,
        format: info.format_display_value,
        allow_right_click_input: info.allow_right_click_input,
        listeners: info.callback.into_iter().collect(),
        tips: Tips { tooltip: info.tooltip, disabled_tooltip: info.disabled_tooltip },
        disabled: info.disabled,
        ball: Anim::new(0.0),
        editing: None,
        edit_focus: FocusRequest::None,
        default: info.default,
    };
    let id = m.insert(Kind::Slider(data), Some(parent));
    m.nodes[id].visible = info.visible;
    m.add_child(parent, id);
    m.register_option(idx, id);
    id
}
pub(crate) fn round(v: f64, rounding: u8) -> f64 {
    if rounding == 0 {
        return v.floor();
    }
    let f = 10f64.powi(rounding as i32);
    (v * f).round() / f
}

/// `Slider:SetValue` (clamps, no rounding; fires listeners when changed).
pub(crate) fn set_value(m: &mut Model, id: NodeId, value: f64) {
    let Some(Kind::Slider(s)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    if s.disabled || !value.is_finite() {
        return;
    }
    let v = value.clamp(s.min, s.max);
    if v == s.value {
        return;
    }
    s.value = v;
    m.pending.push(Pending::Number(id, v));
}

pub(crate) fn set_min(m: &mut Model, id: NodeId, min: f64) {
    let (max, value) = match &m.nodes[id].kind { Kind::Slider(s) => (s.max, s.value), _ => return };
    assert!(min < max, "Min value cannot be greater than the current max value.");
    set_value(m, id, value.clamp(min, max));
    if let Kind::Slider(s) = &mut m.nodes[id].kind {
        s.min = min;
    }
}

pub(crate) fn set_max(m: &mut Model, id: NodeId, max: f64) {
    let (min, value) = match &m.nodes[id].kind { Kind::Slider(s) => (s.min, s.value), _ => return };
    assert!(max > min, "Max value cannot be less than the current min value.");
    set_value(m, id, value.clamp(min, max));
    if let Kind::Slider(s) = &mut m.nodes[id].kind {
        s.max = max;
    }
}

fn fmt_num(v: f64, rounding: u8) -> String {
    if rounding == 0 { format!("{}", v.floor() as i64) } else { format!("{:.*}", rounding as usize, v) }
}

pub(crate) fn display_text(s: &SliderData) -> String {
    if let Some(f) = &s.format {
        return f(s.value);
    }
    let val = fmt_num(s.value, s.rounding);
    if s.compact {
        format!("{}: {}{}{}", s.text, s.prefix, val, s.suffix)
    } else if s.hide_max {
        format!("{}{}{}", s.prefix, val, s.suffix)
    } else {
        format!("{}{}{}/{}{}{}", s.prefix, val, s.suffix, s.prefix, fmt_num(s.max, s.rounding), s.suffix)
    }
}

pub(crate) fn height(m: &Model, id: NodeId, cx: &Cx) -> f32 {
    match &m.nodes[id].kind {
        Kind::Slider(s) if s.compact => cx.px(15.0),
        _ => cx.px(22.0 + BAR_H + BALL_MARGIN),
    }
}

/// Live filtering of the right-click entry box (Lua rules).
fn filter_edit(prev: &str, new: &str, s: &SliderData) -> String {
    if new.is_empty() || new == "-" {
        return new.to_owned();
    }
    if s.rounding == 0 && new.contains('.') {
        return prev.to_owned();
    }
    let Ok(n) = new.parse::<f64>() else { return prev.to_owned() };
    if let Some(dec) = new.split('.').nth(1) {
        if dec.len() > s.rounding as usize {
            return prev.to_owned();
        }
    }
    if n > s.max {
        return fmt_num(s.max, s.rounding);
    }
    if n < s.min {
        return fmt_num(s.min, s.rounding);
    }
    new.to_owned()
}

pub(crate) fn pass(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let h = height(m, id, cx);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    m.nodes[id].rect = r;
    let field = cx.field(id, 0).cloned();

    // Editing box lifecycle.
    let commit = {
        let Kind::Slider(s) = &mut m.nodes[id].kind else { return 0.0 };
        let mut commit = None;
        if let (Some(f), Some(prev)) = (&field, s.editing.clone()) {
            let filtered = filter_edit(&prev, &f.text, s);
            s.editing = Some(filtered.clone());
            if f.lost_focus || f.submitted {
                s.editing = None;
                if let Ok(n) = filtered.parse::<f64>() {
                    commit = Some(round(n, s.rounding));
                }
            }
        }
        commit
    };
    if let Some(v) = commit {
        set_value(m, id, v);
    }
    let Kind::Slider(s) = &m.nodes[id].kind else { return 0.0 };
    let (text, value, min, max, rounding, compact, disabled, tips, allow_rc, editing) =
        (s.text.clone(), s.value, s.min, s.max, s.rounding, s.compact, s.disabled, s.tips.clone(), s.allow_right_click_input, s.editing.clone());
    let display = display_text(s);
    let frac = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) as f32 } else { 0.0 };

    let bar = if compact {
        Rect::from_min_size(Pos2::new(x, r.max.y - cx.px(15.0)), Vec2::new(w, cx.px(15.0)))
    } else {
        Rect::from_min_size(Pos2::new(x, r.max.y - cx.px(BALL_MARGIN) - cx.px(BAR_H)), Vec2::new(w, cx.px(BAR_H)))
    };
    let hovered_bar = cx.hovered(bar) && !disabled;
    let capturing = m.capture.map(|c| c.node == id && c.sub == cap::SLIDER && c.data[3] == 0.0).unwrap_or(false);

    // Start drag / right-click edit.
    if !disabled && m.capture.is_none() && cx.hovered(bar) && editing.is_none() {
        if cx.pressed(MouseButton::Left) {
            m.capture = Some(Capture { node: id, sub: cap::SLIDER, start: cx.pointer().unwrap(), data: [0.0; 4], since: cx.time });
        } else if allow_rc && cx.pressed(MouseButton::Right) {
            if let Kind::Slider(s) = &mut m.nodes[id].kind {
                s.editing = Some(fmt_num(value, rounding));
                s.edit_focus = FocusRequest::Take;
            }
        }
    }
    if capturing || (m.capture.map(|c| c.node == id && c.sub == cap::SLIDER).unwrap_or(false)) {
        if let Some(p) = cx.pointer() {
            let t = ((p.x - bar.min.x) / bar.width()).clamp(0.0, 1.0) as f64;
            let v = round(min + (max - min) * t, rounding);
            set_value(m, id, v);
        }
    }
    let Kind::Slider(s) = &m.nodes[id].kind else { return 0.0 };
    let value = s.value;
    let frac = if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) as f32 } else { frac };
    let display = if s.value != value { display_text(s) } else { display };
    let editing = s.editing.clone();

    let text_t = if disabled { 0.8 } else { 0.0 };
    if compact {
        cx.rect(bar, cx.pill(bar), cx.col(cx.sch.main, 0.0));
        let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, bar.height()));
        cx.rect(fill, cx.pill(bar), cx.col(if disabled { cx.sch.outline } else { cx.sch.accent }, 0.0));
        cx.stroke(bar, cx.pill(bar), 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Inside);
        if editing.is_none() {
            let shadow = cx.col(cx.sch.dark, 0.0);
            let tr = bar.translate(Vec2::new(0.0, cx.px(1.0)));
            cx.text(tr, &display, 14.0, shadow, Align::Center, false, false);
            cx.text(bar, &display, 14.0, cx.col(cx.sch.font_color, text_t), Align::Center, false, false);
        }
    } else {
        let label_r = Rect::from_min_size(r.min, Vec2::new(w - cx.px(70.0), cx.px(14.0)));
        let t = cx.truncate(&text, 14.0, label_r.width(), true);
        cx.text(label_r, &t, 14.0, cx.col(cx.sch.font_color, text_t), Align::Min, false, true);
        let value_r = Rect::from_min_size(Pos2::new(r.max.x - cx.px(70.0), r.min.y), Vec2::new(cx.px(70.0), cx.px(14.0)));
        if editing.is_none() {
            cx.text(value_r, &display, 14.0, cx.col(cx.sch.font_color, if disabled { 0.8 } else { 0.4 }), Align::Max, false, false);
        }
        // Track with gradient, inner ring, fill, ball.
        let pill = cx.pill(bar);
        let shape = draw::gradient_rounded_rect(bar, pill, cx.col(TRACK_GRADIENT.0, 0.0), cx.col(TRACK_GRADIENT.1, 0.0), true);
        cx.push(shape);
        cx.stroke(bar, pill, 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Inside);
        let ball_v = {
            let Kind::Slider(s) = &mut m.nodes[id].kind else { return 0.0 };
            let active = !disabled && (capturing || hovered_bar);
            let mut a = s.ball;
            let v = cx.tween(&mut a, if active { 1.0 } else { 0.0 }, tw::SLIDER_BALL);
            s.ball = a;
            v
        };
        let ball_size = cx.px(BALL + (BALL_ACTIVE - BALL) * ball_v);
        let edge_x = bar.min.x + bar.width() * frac + (0.5 - frac) * ball_size;
        let fill = Rect::from_min_max(bar.min, Pos2::new(edge_x.max(bar.min.x), bar.max.y));
        cx.rect(fill, pill, cx.col(if disabled { cx.sch.outline } else { cx.sch.accent }, 0.0));
        let ring = bar.shrink(cx.px(1.0));
        cx.stroke(ring, cx.pill(ring), 1.0, cx.col(cx.sch.dark, 0.7), StrokeKind::Inside);
        let center = Pos2::new(edge_x, bar.center().y);
        cx.push(epaint::Shape::circle_filled(center + Vec2::new(0.0, cx.px(1.0)), ball_size / 2.0, cx.col(cx.sch.dark, 0.55)));
        cx.push(epaint::Shape::circle_filled(center, ball_size / 2.0, cx.col(cx.sch.font_color, 0.0)));
        cx.push(epaint::Shape::circle_stroke(center, ball_size / 2.0, epaint::Stroke::new(cx.px(1.0), cx.col(cx.sch.dark, 0.75))));
    }

    // Right-click entry field.
    if let Some(edit) = editing {
        let fr = if compact { bar.shrink2(Vec2::new(cx.px(8.0), 0.0)) } else { Rect::from_min_size(Pos2::new(r.max.x - cx.px(70.0), r.min.y), Vec2::new(cx.px(70.0), cx.px(14.0))) };
        let focus = {
            let Kind::Slider(s) = &mut m.nodes[id].kind else { return 0.0 };
            std::mem::replace(&mut s.edit_focus, FocusRequest::None)
        };
        cx.request_field(TextFieldRequest {
            id,
            sub: 0,
            rect: fr,
            text: edit,
            placeholder: String::new(),
            font: cx.font(14.0),
            color: cx.col(cx.sch.font_color, 0.0),
            placeholder_color: cx.col(cx.sch.placeholder(), 0.0),
            focus,
            numeric: true,
            max_len: None,
            clear_on_focus: false,
            password: false,
            layer: cx.layer,
        });
        cx.animate();
    }
    if let Some(tt) = tips.text(disabled) {
        if cx.hovered(bar) {
            cx.tooltip(id, tt);
        }
    }
    let _ = Corners::ZERO;
    h
}
