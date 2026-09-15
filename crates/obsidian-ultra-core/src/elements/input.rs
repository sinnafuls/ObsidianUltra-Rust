//! Text input.

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::frame::{Cx, FocusRequest, Layer, TextFieldRequest};
use crate::handles::info::InputInfo;
use crate::node::*;
use crate::tween::{info as tw, Anim};
use crate::ui::{Model, Pending};

pub(crate) fn create(m: &mut Model, parent: NodeId, idx: &str, info: InputInfo) -> NodeId {
    let finished = info.finished || info.verify_value.is_some();
    let mut data = InputData {
        text: info.text,
        value: String::new(),
        finished,
        numeric: info.numeric,
        clear_text_on_focus: info.clear_text_on_focus,
        clear_text_on_blur: info.clear_text_on_blur,
        placeholder: info.placeholder,
        allow_empty: info.allow_empty,
        empty_reset: info.empty_reset,
        max_length: info.max_length,
        verify: info.verify_value,
        listeners: info.callback.into_iter().collect(),
        tips: Tips { tooltip: info.tooltip, disabled_tooltip: info.disabled_tooltip },
        disabled: info.disabled,
        focus: Anim::new(0.0),
        default: info.default.clone(),
        editing: String::new(),
    };
    // Defaults go through the pipeline without firing listeners.
    let v = pipeline(&data, &info.default);
    data.value = v.clone();
    data.editing = v.clone();
    data.default = v;
    let id = m.insert(Kind::Input(data), Some(parent));
    m.nodes[id].visible = info.visible;
    m.add_child(parent, id);
    m.register_option(idx, id);
    id
}

/// The `SetValue` pipeline; returns the accepted value.
fn pipeline(d: &InputData, text: &str) -> String {
    let mut text = text.to_owned();
    if !d.allow_empty && text.trim().is_empty() {
        text = d.empty_reset.clone();
    }
    if let Some(max) = d.max_length {
        if text.chars().count() > max {
            text = text.chars().take(max).collect();
        }
    }
    if d.numeric && !text.is_empty() && text.parse::<f64>().is_err() {
        return d.value.clone();
    }
    if let Some(v) = &d.verify {
        if text != d.empty_reset && !v(&text) {
            text = d.empty_reset.clone();
        }
    }
    text
}
pub(crate) fn set_value(m: &mut Model, id: NodeId, text: &str) {
    let Some(Kind::Input(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    let v = pipeline(d, text);
    d.value = v.clone();
    d.editing = v.clone();
    if !d.disabled {
        m.pending.push(Pending::Text(id, v));
    }
}

pub(crate) fn height(_m: &Model, _id: NodeId, cx: &Cx) -> f32 {
    cx.px(39.0)
}

pub(crate) fn pass(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let h = cx.px(39.0);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    m.nodes[id].rect = r;
    let field = cx.field(id, 0).cloned();
    let focused = field.as_ref().map(|f| f.focused).unwrap_or(false);

    // Apply field changes.
    let (commit, focus_v) = {
        let Kind::Input(d) = &mut m.nodes[id].kind else { return 0.0 };
        let mut commit: Option<String> = None;
        if let Some(f) = &field {
            if d.finished {
                if f.submitted {
                    commit = Some(f.text.clone());
                } else if f.lost_focus {
                    if d.clear_text_on_blur {
                        d.editing = d.value.clone();
                    } else {
                        d.editing = f.text.clone();
                    }
                } else {
                    d.editing = f.text.clone();
                }
            } else if f.text != d.editing {
                d.editing = f.text.clone();
                if f.text != d.value {
                    commit = Some(f.text.clone());
                }
            }
        }
        let mut a = d.focus;
        let fv = cx.tween(&mut a, if focused && !d.disabled { 1.0 } else { 0.0 }, tw::HOVER);
        d.focus = a;
        (commit, fv)
    };
    if let Some(c) = commit {
        set_value(m, id, &c);
    }
    let Kind::Input(d) = &m.nodes[id].kind else { return 0.0 };
    let (label, placeholder, disabled, tips, editing, clear_on_focus) = (d.text.clone(), d.placeholder.clone(), d.disabled, d.tips.clone(), d.editing.clone(), d.clear_text_on_focus);

    let label_r = Rect::from_min_size(r.min, Vec2::new(w, cx.px(14.0)));
    let align = if cx.key_tab { Align::Center } else { Align::Min };
    cx.text(label_r, &label, 14.0, cx.col(cx.sch.font_color, if disabled { 0.8 } else { 0.0 }), align, false, true);
    let box_r = Rect::from_min_size(Pos2::new(x, r.max.y - cx.px(21.0)), Vec2::new(w, cx.px(21.0)));
    cx.rect(box_r, cx.r2(), cx.col(cx.sch.main, 0.0));
    let stroke_c = crate::color::lerp(cx.sch.outline, cx.sch.accent, focus_v);
    cx.stroke(box_r, cx.r2(), 1.0, cx.col(stroke_c, 0.0), StrokeKind::Inside);
    let field_r = Rect::from_min_max(Pos2::new(box_r.min.x + cx.px(8.0), box_r.min.y + cx.px(4.0)), Pos2::new(box_r.max.x - cx.px(8.0), box_r.max.y - cx.px(3.0)));
    let color = cx.col(cx.sch.font_color, if disabled { 0.8 } else { 0.0 });
    if !disabled && m.open && cx.fade > 0.99 && !cx.content_covered && cx.clip.intersects(box_r) && !crate::window::covered(m, cx.layer, box_r) {
        cx.request_field(TextFieldRequest {
            id,
            sub: 0,
            rect: field_r,
            text: editing,
            placeholder,
            font: cx.font(14.0),
            color,
            placeholder_color: cx.col(cx.sch.placeholder(), 0.0),
            focus: FocusRequest::None,
            numeric: false,
            max_len: None,
            clear_on_focus,
            password: false,
            layer: cx.layer,
        });
    } else {
        let shown = if editing.is_empty() { placeholder.clone() } else { editing.clone() };
        let c = if editing.is_empty() { cx.col(cx.sch.placeholder(), 0.0) } else { color };
        let t = cx.truncate(&shown, 14.0, field_r.width(), false);
        cx.text(field_r, &t, 14.0, c, Align::Min, false, false);
    }
    if let Some(tt) = tips.text(disabled) {
        if cx.hovered(box_r) {
            cx.tooltip(id, tt);
        }
    }
    let _ = Layer::Window;
    h
}
