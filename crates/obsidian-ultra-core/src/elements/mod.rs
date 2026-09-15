//! Elements: constructors, per-frame passes and shared stacking helpers.

pub(crate) mod basic;
pub(crate) mod colorpicker;
pub(crate) mod dependency;
pub(crate) mod dropdown;
pub(crate) mod input;
pub(crate) mod keypicker;
pub(crate) mod media;
pub(crate) mod menu;
pub(crate) mod priority;
pub(crate) mod slider;
pub(crate) mod toggle;

use epaint::{Pos2, Rect, Vec2};

use crate::frame::Cx;
use crate::node::*;
use crate::ui::Model;

/// Height an element will occupy at width `w` (design-scaled points). Hidden elements -> 0.
/// A button row is shown if the button or its sub-button is shown (search may hide one half).
fn row_shown(m: &Model, n: &Node) -> bool {
    n.shown() || (!n.destroyed && matches!(&n.kind, Kind::Button(b) if b.sub.and_then(|s| m.nodes.get(s)).map(|s| s.shown()).unwrap_or(false)))
}

pub(crate) fn height(m: &mut Model, id: NodeId, cx: &mut Cx, w: f32) -> f32 {
    let Some(n) = m.nodes.get(id) else { return 0.0 };
    if !row_shown(m, n) {
        return 0.0;
    }
    match &n.kind {
        Kind::Divider(_) => basic::divider_height(m, id, cx),
        Kind::Label(_) => basic::label_height(m, id, cx, w),
        Kind::Button(_) => basic::button_height(m, id, cx),
        Kind::Toggle(_) => toggle::height(m, id, cx),
        Kind::Input(_) => input::height(m, id, cx),
        Kind::Slider(_) => slider::height(m, id, cx),
        Kind::Dropdown(_) => dropdown::height(m, id, cx),
        Kind::Priority(_) => priority::height(m, id, cx),
        Kind::Image(_) => media::image_height(m, id, cx),
        Kind::ProfileCard(_) => media::profile_height(m, id, cx, w),
        Kind::Custom(_) => media::custom_height(m, id, cx),
        Kind::DepBox(_) => dependency::dep_box_height(m, id, cx, w),
        Kind::Tabbox(_) => {
            // Nested tabbox inside a groupbox: measured like a box (+ holder padding).
            let h = tabbox_measure(m, id, cx, w);
            if h > 0.0 { h + cx.px(8.0) } else { 0.0 }
        }
        Kind::KeyBox(_) => cx.px(21.0),
        _ => 0.0,
    }
}

fn tabbox_measure(m: &mut Model, id: NodeId, cx: &mut Cx, w: f32) -> f32 {
    let Kind::Tabbox(t) = &m.nodes[id].kind else { return 0.0 };
    if t.pop.popped {
        return cx.px(34.0);
    }
    let active = t.active;
    let children = active.map(|a| m.nodes[a].children.clone()).unwrap_or_default();
    let inner_w = w - cx.px(14.0);
    let content = if active.is_some() { measure_children(m, &children, cx, inner_w, cx.px(8.0)) + cx.px(14.0) } else { 0.0 };
    cx.px(34.0) + cx.px(1.0) + content
}

/// Lay out one element at `(x, y)` with width `w`; returns the height used.
pub(crate) fn pass(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let Some(n) = m.nodes.get(id) else { return 0.0 };
    if !row_shown(m, n) {
        return 0.0;
    }
    let h = match &n.kind {
        Kind::Divider(_) => basic::pass_divider(m, id, cx, x, y, w),
        Kind::Label(_) => basic::pass_label(m, id, cx, x, y, w),
        Kind::Button(_) => basic::pass_button(m, id, cx, x, y, w),
        Kind::Toggle(_) => toggle::pass(m, id, cx, x, y, w),
        Kind::Input(_) => input::pass(m, id, cx, x, y, w),
        Kind::Slider(_) => slider::pass(m, id, cx, x, y, w),
        Kind::Dropdown(_) => dropdown::pass(m, id, cx, x, y, w),
        Kind::Priority(_) => priority::pass(m, id, cx, x, y, w),
        Kind::Image(_) => media::pass_image(m, id, cx, x, y, w),
        Kind::ProfileCard(_) => media::pass_profile(m, id, cx, x, y, w),
        Kind::Custom(_) => media::pass_custom(m, id, cx, x, y, w),
        Kind::DepBox(_) => dependency::pass_dep_box(m, id, cx, x, y, w),
        Kind::Tabbox(_) => {
            let popped = matches!(&m.nodes[id].kind, Kind::Tabbox(t) if t.pop.popped);
            let h = if popped {
                crate::window::boxes::pass_box(m, id, cx, x, y + cx.px(4.0), w)
            } else {
                crate::window::boxes::draw_tabbox(m, id, cx, x, y + cx.px(4.0), w, false)
            };
            if h > 0.0 { h + cx.px(8.0) } else { 0.0 }
        }
        Kind::KeyBox(_) => crate::window::tabs::pass_key_box(m, id, cx, x, y, w),
        _ => 0.0,
    };
    if let Some(n) = m.nodes.get_mut(id) {
        if n.rect == Rect::NOTHING || !matches!(n.kind, Kind::DepBox(_)) {
            n.rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
        }
    }
    h
}

/// Total height of a vertical stack of children with `gap` between shown items.
pub(crate) fn measure_children(m: &mut Model, children: &[NodeId], cx: &mut Cx, w: f32, gap: f32) -> f32 {
    let mut total = 0.0;
    let mut count = 0;
    for c in children {
        let h = height(m, *c, cx, w);
        if h > 0.0 {
            total += h;
            count += 1;
        }
    }
    if count > 1 {
        total += gap * (count - 1) as f32;
    }
    total
}

/// Lay out a vertical stack of children. `key_tab` centers label text.
pub(crate) fn pass_children(m: &mut Model, children: &[NodeId], cx: &mut Cx, x: f32, y: f32, w: f32, gap: f32, key_tab: bool) -> f32 {
    let prev = cx.key_tab;
    cx.key_tab = key_tab;
    let mut cy = y;
    let mut any = false;
    for c in children {
        if !m.alive(*c) {
            continue;
        }
        let h = pass(m, *c, cx, x, cy, w);
        if h > 0.0 {
            cy += h + gap;
            any = true;
        }
    }
    cx.key_tab = prev;
    if any { cy - y - gap } else { 0.0 }
}

/// Shared "disabled" transparency helper.
pub(crate) fn text_transparency(disabled: bool, hover: f32, on: bool) -> f32 {
    if disabled {
        0.8
    } else if on {
        0.0
    } else {
        0.4 * (1.0 - hover)
    }
}
