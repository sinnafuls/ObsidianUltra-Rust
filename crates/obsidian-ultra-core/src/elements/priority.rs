//! Priority dropdown (drag-to-rank list).

use std::f32::consts::PI;

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use super::menu::{self, MenuCorners, MenuSpec};
use crate::draw::Corners;
use crate::frame::{Cx, FocusRequest, Layer, TextFieldRequest};
use crate::handles::info::PriorityDropdownInfo;
use crate::input::MouseButton;
use crate::node::*;
use crate::search;
use crate::tween::{info as tw, Anim};
use crate::types::IconRef;
use crate::ui::{Model, Pending};

const ROW_H: f32 = 24.0;
const XROW_H: f32 = 34.0;
const BOX_H: f32 = 21.0;
const HEADER_H: f32 = 34.0;
const SEARCH_H: f32 = 26.0;
const LIST_PAD: f32 = 10.0;
/// Auto-scroll band (design px) at the list edges while dragging, and its speed per frame.
const EDGE: f32 = 26.0;
const SPEED: f32 = 9.0;

/// Known values from `order` first (deduplicated), then the rest of `values`.
pub(crate) fn resolve_order(values: &[String], order: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(values.len());
    for v in order {
        if values.contains(v) && !out.contains(v) {
            out.push(v.clone());
        }
    }
    for v in values {
        if !out.contains(v) {
            out.push(v.clone());
        }
    }
    out
}

pub(crate) fn create(m: &mut Model, parent: NodeId, idx: &str, info: PriorityDropdownInfo) -> NodeId {
    let value = resolve_order(&info.values, &info.default);
    let data = PriorityData {
        text: info.text,
        values: info.values,
        value: value.clone(),
        max_visible: info.max_visible_dropdown_items.max(1),
        searchable: info.searchable,
        expandable: info.expandable,
        format: info.format_display_value,
        listeners: info.callback.into_iter().collect(),
        tips: Tips { tooltip: info.tooltip, disabled_tooltip: info.disabled_tooltip },
        disabled: info.disabled,
        menu: MenuState::NONE,
        scroll: Default::default(),
        search: String::new(),
        search_focus: Default::default(),
        expanded: false,
        expand: Default::default(),
        expand_search: String::new(),
        expand_scroll: Default::default(),
        drag: None,
        row_y: Vec::new(),
        expand_row_y: Vec::new(),
        default: value,
        arrow: Default::default(),
        focus: Default::default(),
    };
    let id = m.insert(Kind::Priority(Box::new(data)), Some(parent));
    m.nodes[id].visible = info.visible;
    m.add_child(parent, id);
    m.register_option(idx, id);
    id
}

// ----- helpers -------------------------------------------------------------------------------
fn format(p: &PriorityData, v: &str) -> String {
    match &p.format {
        Some(f) => f(v),
        None => v.to_owned(),
    }
}

/// Collapsed text: `first  (+N)`.
fn display(p: &PriorityData) -> String {
    let mut s = p.value.first().map(|v| format(p, v)).unwrap_or_default();
    if p.value.len() > 1 {
        s.push_str(&format!("  (+{})", p.value.len() - 1));
    }
    super::dropdown::truncate_display(&s)
}

/// Normalized search of a view; empty while dragging.
fn query(p: &PriorityData, expanded: bool) -> String {
    if p.drag.is_some() {
        return String::new();
    }
    search::normalize(if expanded { &p.expand_search } else { &p.search })
}

/// Per-value visibility for a view's search.
fn visibility(p: &PriorityData, expanded: bool) -> Vec<bool> {
    let q = query(p, expanded);
    p.value.iter().map(|v| q.is_empty() || search::fuzzy_score(&format(p, v).to_lowercase(), &q).is_some()).collect()
}

/// Pack visible rows top-down, tweening rows to their slot (except the dragged one).
/// Row animations are index-aligned with `value` and hold design-unit y offsets.
/// Returns per-value visibility and the visible count.
fn relayout(p: &mut PriorityData, expanded: bool, animate: bool, time: f64) -> (Vec<bool>, usize) {
    let vis = visibility(p, expanded);
    let skip = p.drag.as_ref().filter(|d| d.expanded == expanded).and_then(|d| p.value.iter().position(|v| *v == d.value));
    let item_h = if expanded { XROW_H } else { ROW_H };
    let n = p.value.len();
    let anims = if expanded { &mut p.expand_row_y } else { &mut p.row_y };
    let rebuilt = anims.len() != n;
    if rebuilt {
        anims.clear();
        anims.resize(n, Anim::new(0.0));
    }
    let mut count = 0usize;
    for i in 0..n {
        // Hidden rows jump to their would-be slot so they do not fly in when the search clears.
        let target = count as f32 * item_h;
        if !vis[i] {
            anims[i].snap(target);
            continue;
        }
        count += 1;
        if Some(i) == skip {
            continue;
        }
        if animate && !rebuilt {
            anims[i].set(target, time, tw::HOVER.0, tw::HOVER.1);
        } else {
            anims[i].snap(target);
        }
    }
    (vis, count)
}

/// Move the value at `from` to `to`, keeping both views' row animations index-aligned.
fn move_value(p: &mut PriorityData, from: usize, to: usize) {
    if from == to || from >= p.value.len() || to >= p.value.len() {
        return;
    }
    let v = p.value.remove(from);
    p.value.insert(to, v);
    for anims in [&mut p.row_y, &mut p.expand_row_y] {
        if anims.len() == p.value.len() {
            let a = anims.remove(from);
            anims.insert(to, a);
        }
    }
}

fn fire(m: &mut Model, id: NodeId) {
    let Some(Kind::Priority(p)) = m.nodes.get(id).map(|n| &n.kind) else { return };
    let v = p.value.clone();
    m.pending.push(Pending::List(id, v));
}

// ----- public mutators -----------------------------------------------------------------------

pub(crate) fn set_value(m: &mut Model, id: NodeId, order: Vec<String>) {
    let fire_it = {
        let Some(Kind::Priority(p)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
        p.value = resolve_order(&p.values, &order);
        p.drag = None;
        // `BuildList` recreates the rows: no slide animation.
        p.row_y.clear();
        p.expand_row_y.clear();
        !p.disabled
    };
    if m.capture.map(|c| c.node == id && c.sub == cap::PRIORITY).unwrap_or(false) {
        m.capture = None;
    }
    if fire_it {
        fire(m, id);
    }
}

pub(crate) fn set_values(m: &mut Model, id: NodeId, values: Vec<String>) {
    let current = {
        let Some(Kind::Priority(p)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
        p.values = values;
        p.value.clone()
    };
    set_value(m, id, current);
}

pub(crate) fn set_expanded(m: &mut Model, id: NodeId, expanded: bool) {
    let time = m.time;
    if expanded {
        match m.nodes.get(id).map(|n| &n.kind) {
            Some(Kind::Priority(p)) if !p.disabled && p.expandable && !p.expanded => {}
            _ => return,
        }
        if let Some(other) = m.active_expanded {
            if other != id {
                super::dropdown::collapse_other(m, other);
            }
        }
        let mut st = match &m.nodes[id].kind {
            Kind::Priority(p) => p.menu,
            _ => return,
        };
        menu::close(m, id, 0, &mut st);
        let Kind::Priority(p) = &mut m.nodes[id].kind else { return };
        p.menu = st;
        // `opened_at` doubles as the expand time so the opening click is not read as an overlay click.
        p.menu.opened_at = time;
        p.search.clear();
        p.expand_search.clear();
        p.expand_scroll.offset = 0.0;
        p.expand_row_y.clear();
        p.expanded = true;
        p.expand.play(0.94, 1.0, time, tw::DROPDOWN.0, tw::DROPDOWN.1);
        m.active_expanded = Some(id);
    } else {
        let Some(Kind::Priority(p)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
        if !p.expanded {
            return;
        }
        p.expanded = false;
        p.expand.set(0.96, time, tw::DROPDOWN.0, tw::DROPDOWN.1);
        // `m.active_expanded` is released by `pass_expanded` once the collapse animation ends.
    }
}

pub(crate) fn set_disabled(m: &mut Model, id: NodeId, disabled: bool) {
    let mut st = match m.nodes.get(id).map(|n| &n.kind) {
        Some(Kind::Priority(p)) => p.menu,
        _ => return,
    };
    menu::close(m, id, 0, &mut st);
    if let Kind::Priority(p) = &mut m.nodes[id].kind {
        p.disabled = disabled;
        p.menu = st;
        p.drag = None;
    }
    if disabled {
        set_expanded(m, id, false);
    }
}

pub(crate) fn height(m: &Model, id: NodeId, cx: &Cx) -> f32 {
    match &m.nodes[id].kind {
        Kind::Priority(p) => cx.px(if p.text.is_some() { 39.0 } else { 21.0 }),
        _ => 0.0,
    }
}

// ----- shared list pass ---------------------------------------------------------------------

/// Geometry of one rank list view (inline menu or expanded panel).
struct View {
    expanded: bool,
    /// Visible list area (rows are clipped to it).
    list: Rect,
    /// Row pitch in points.
    pitch: f32,
    /// Drawn row height in points (`pitch` inline, `pitch - 4` expanded).
    row_h: f32,
}

/// Lay out, draw and interact with the rank rows of one view. Returns true when a drag ended.
fn pass_rows(m: &mut Model, id: NodeId, cx: &mut Cx, view: View, capture: Option<Capture>) -> bool {
    let captured = capture.map(|c| c.node == id && c.sub == cap::PRIORITY).unwrap_or(false);
    let pointer = cx.pointer();
    let wheel = cx.wheel_over(view.list);
    let time = cx.time;
    let mut drag_ended = false;
    let mut start_drag: Option<(String, f32)> = None;
    let Kind::Priority(p) = &mut m.nodes[id].kind else { return false };
    let n = p.value.len();
    let disabled = p.disabled;

    // Finish a released drag before laying out.
    if p.drag.as_ref().map(|d| d.expanded == view.expanded).unwrap_or(false) && !captured {
        p.drag = None;
        drag_ended = true;
    }
    let dragging = p.drag.as_ref().filter(|d| d.expanded == view.expanded).map(|d| d.value.clone());
    let (vis, count) = relayout(p, view.expanded, true, time);
    let searching = !query(p, view.expanded).is_empty();
    let can_drag = !searching && !disabled;

    let scroll = if view.expanded { &mut p.expand_scroll } else { &mut p.scroll };
    scroll.view = view.list.height();
    scroll.content = count as f32 * view.pitch;
    if wheel != 0.0 && capture.is_none() {
        scroll.wheel(wheel);
    }
    // Auto-scroll near the edges while dragging in this view.
    if let (Some(py), true) = (pointer.map(|q| q.y), dragging.is_some() && captured) {
        if scroll.scrollable() {
            if py < view.list.min.y + cx.px(EDGE) {
                scroll.offset -= cx.px(SPEED);
            } else if py > view.list.max.y - cx.px(EDGE) {
                scroll.offset += cx.px(SPEED);
            }
        }
    }
    scroll.clamp();
    let offset = scroll.offset;

    // Drag: follow the pointer and reorder live.
    let mut drag_y: Option<f32> = None;
    if let (Some(dv), true, Some(ptr)) = (dragging.as_ref(), captured, pointer) {
        if let Some(d) = &mut p.drag {
            d.pointer_y = ptr.y;
        }
        let grab = p.drag.as_ref().map(|d| d.grab_offset).unwrap_or(0.0);
        let max_y = (n.saturating_sub(1)) as f32 * view.pitch;
        let vis_y = (ptr.y - view.list.min.y + offset - grab).clamp(0.0, max_y);
        let target = ((vis_y / view.pitch + 0.5).floor().max(0.0) as usize).min(n.saturating_sub(1));
        if let Some(cur) = p.value.iter().position(|v| v == dv) {
            if cur != target {
                move_value(p, cur, target);
                relayout(p, view.expanded, true, time);
            }
        }
        drag_y = Some(vis_y);
        cx.animate();
    }

    // Rows in two sweeps: the dragged row is drawn last so it floats above the others.
    let (grip_x, grip_sz, num_x, num_w, num_size, val_x, val_pad, val_size, val_t) =
        if view.expanded { (16.0, 16.0, 32.0, 26.0, 14.0, 62.0, 70.0, 15.0, 0.25) } else { (12.0, 14.0, 24.0, 20.0, 13.0, 46.0, 52.0, 14.0, 0.35) };
    let corners = if view.expanded { Corners::same(cx.r2()) } else { Corners::ZERO };
    let grip_icon = IconRef::Lucide("grip-vertical".into());
    let grip_fallback = IconRef::Lucide("menu".into());
    for sweep in 0..2 {
        for i in 0..n {
            if !vis[i] {
                continue;
            }
            let is_drag = dragging.as_deref() == Some(p.value[i].as_str());
            if (sweep == 1) != is_drag {
                continue;
            }
            let anim = if view.expanded { p.expand_row_y[i] } else { p.row_y[i] };
            let y = match (is_drag, drag_y) {
                (true, Some(dy)) => dy,
                _ => cx.px(anim.value(time)),
            };
            if anim.active(time) {
                cx.animate();
            }
            let row_r = Rect::from_min_size(Pos2::new(view.list.min.x, view.list.min.y + y - offset), Vec2::new(view.list.width(), view.row_h));
            if !row_r.intersects(view.list) {
                continue;
            }
            let hovered = cx.hovered(row_r) && can_drag && dragging.is_none();
            let (bg_t, grip_t) = if is_drag {
                (0.6, 0.1)
            } else if hovered {
                (if view.expanded { 0.85 } else { 0.9 }, 0.1)
            } else {
                (1.0, if view.expanded { 0.55 } else { 0.6 })
            };
            if bg_t < 0.999 {
                cx.rect(row_r, corners, cx.col(cx.sch.main, bg_t));
            }
            if view.expanded {
                cx.stroke(row_r, corners, 1.0, cx.col(cx.sch.outline, 0.6), StrokeKind::Inside);
            }
            let grip_r = Rect::from_center_size(Pos2::new(row_r.min.x + cx.px(grip_x), row_r.center().y), Vec2::splat(cx.px(grip_sz)));
            if !cx.icon(&grip_icon, grip_r, cx.col(cx.sch.font_color, grip_t)) {
                cx.icon(&grip_fallback, grip_r, cx.col(cx.sch.font_color, grip_t));
            }
            let num_r = Rect::from_min_size(Pos2::new(row_r.min.x + cx.px(num_x), row_r.min.y), Vec2::new(cx.px(num_w), row_r.height()));
            cx.text(num_r, &(i + 1).to_string(), num_size, cx.col(cx.sch.font_color, 0.4), Align::Min, false, false);
            let val_r = Rect::from_min_max(Pos2::new(row_r.min.x + cx.px(val_x), row_r.min.y), Pos2::new(row_r.max.x - cx.px(val_pad - val_x), row_r.max.y));
            let label = format(p, &p.value[i]);
            let t = cx.truncate(&label, val_size, val_r.width(), false);
            cx.text(val_r, &t, val_size, cx.col(cx.sch.font_color, val_t), Align::Min, false, false);
            if hovered && capture.is_none() && p.drag.is_none() && cx.pressed(MouseButton::Left) {
                if let Some(ptr) = pointer {
                    start_drag = Some((p.value[i].clone(), ptr.y - row_r.min.y));
                }
            }
        }
    }

    // Scrollbar.
    let scroll = if view.expanded { &p.expand_scroll } else { &p.scroll };
    if scroll.scrollable() {
        let bar_w = cx.px(if view.expanded { 3.0 } else { 2.0 });
        let track = Rect::from_min_max(Pos2::new(view.list.max.x - bar_w, view.list.min.y), view.list.max);
        let thumb = scroll.thumb(track);
        cx.rect(thumb, cx.pill(thumb), cx.col(cx.sch.outline, 0.0));
    }

    if let Some((value, grab_offset)) = start_drag {
        p.drag = Some(PriorityDrag { value, grab_offset, expanded: view.expanded, pointer_y: pointer.map(|q| q.y).unwrap_or(0.0) });
        m.capture = Some(Capture { node: id, sub: cap::PRIORITY, start: pointer.unwrap_or(Pos2::ZERO), data: [0.0; 4], since: time });
    }
    drag_ended
}

// ----- collapsed widget + inline list --------------------------------------------------------

pub(crate) fn pass(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let h = height(m, id, cx);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    m.nodes[id].rect = r;
    let box_r = Rect::from_min_size(Pos2::new(x, r.max.y - cx.px(BOX_H)), Vec2::new(w, cx.px(BOX_H)));
    let field = cx.field(id, 0).cloned();
    let focused = field.as_ref().map(|f| f.focused).unwrap_or(false);
    let animate_menu = m.settings.animations.dropdown;
    let capture = m.capture;

    let (text, disabled, tips, searchable, expandable, max_visible, focus_v, search, search_focus, visible_count) = {
        let Kind::Priority(p) = &mut m.nodes[id].kind else { return 0.0 };
        if let Some(f) = &field {
            if p.menu.open && p.searchable && f.text != p.search {
                p.search = f.text.clone();
                p.scroll.offset = 0.0;
            }
        }
        let mut a = p.focus;
        let fv = cx.tween(&mut a, if focused && p.menu.open && !p.disabled { 1.0 } else { 0.0 }, tw::HOVER);
        p.focus = a;
        let count = visibility(p, false).iter().filter(|v| **v).count();
        (
            p.text.clone(),
            p.disabled,
            p.tips.clone(),
            p.searchable,
            p.expandable,
            p.max_visible,
            fv,
            p.search.clone(),
            std::mem::replace(&mut p.search_focus, FocusRequest::None),
            count,
        )
    };

    // Inline list menu.
    let list_h = (visible_count.min(max_visible) as f32) * ROW_H;
    let spec = MenuSpec {
        holder: box_r,
        width: box_r.width(),
        offset: Vec2::new(cx.px(0.5), cx.px(BOX_H + 1.5)),
        height: cx.px(list_h),
        anim: if animate_menu { Some(tw::DROPDOWN) } else { None },
    };
    let mut st = match &m.nodes[id].kind {
        Kind::Priority(p) => p.menu,
        _ => return 0.0,
    };
    let menu_rect = menu::begin(m, id, 0, cx, &mut st, spec);
    let open = st.open;
    let arrow_v = {
        let Kind::Priority(p) = &mut m.nodes[id].kind else { return 0.0 };
        p.menu = st;
        if !open && !p.search.is_empty() {
            p.search.clear();
        }
        if !open && p.drag.as_ref().map(|d| !d.expanded).unwrap_or(false) {
            p.drag = None;
        }
        let mut a = p.arrow;
        let v = cx.tween(&mut a, if open { 1.0 } else { 0.0 }, tw::HOVER);
        p.arrow = a;
        v
    };

    // Display box.
    let corners = if open { Corners::top(cx.r2()) } else { Corners::same(cx.r2()) };
    cx.rect(box_r, corners, cx.col(cx.sch.main, 0.0));
    let stroke_c = crate::color::lerp(cx.sch.outline, cx.sch.accent, focus_v);
    cx.stroke(box_r, corners, 1.0, cx.col(stroke_c, 0.0), StrokeKind::Inside);
    let text_t = if disabled { 0.8 } else { 0.0 };
    if let Some(t) = &text {
        let label_r = Rect::from_min_size(r.min, Vec2::new(w, cx.px(14.0)));
        let align = if cx.key_tab { Align::Center } else { Align::Min };
        cx.text(label_r, t, 14.0, cx.col(cx.sch.font_color, text_t), align, false, true);
    }
    if let Some(tt) = tips.text(disabled) {
        if cx.hovered(box_r) {
            cx.tooltip(id, tt);
        }
    }
    let arrow_r = Rect::from_center_size(Pos2::new(box_r.max.x - cx.px(4.0 + 8.0), box_r.center().y), Vec2::splat(cx.px(16.0)));
    let arrow_t = if disabled { 0.8 } else { 0.5 * (1.0 - arrow_v) };
    cx.icon_rotated(&IconRef::Lucide("chevron-up".into()), arrow_r, cx.col(cx.sch.font_color, arrow_t), PI * arrow_v);

    let mut text_right = box_r.max.x - cx.px(16.0);
    let mut expand_r = Rect::NOTHING;
    let mut expand_clicked = false;
    if expandable {
        expand_r = Rect::from_center_size(Pos2::new(box_r.max.x - cx.px(4.0 + 18.0 + 8.0), box_r.center().y), Vec2::splat(cx.px(16.0)));
        let hovered = cx.hovered(expand_r) && !disabled;
        let hv = {
            let node = &mut m.nodes[id];
            let mut a = node.hover;
            let v = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
            node.hover = a;
            v
        };
        let t = if disabled { 0.8 } else { 0.5 * (1.0 - hv) };
        cx.icon(&IconRef::Lucide("maximize-2".into()), expand_r, cx.col(cx.sch.font_color, t));
        if hovered {
            cx.tooltip(id, "Expand");
        }
        if !disabled && cx.clicked(expand_r) {
            expand_clicked = true;
        }
        text_right = box_r.max.x - cx.px(38.0);
    }

    let display_str = match &m.nodes[id].kind {
        Kind::Priority(p) => display(p),
        _ => return 0.0,
    };
    let text_left = box_r.min.x + cx.px(8.0);
    let text_r = Rect::from_min_max(Pos2::new(text_left, box_r.min.y), Pos2::new(text_right, box_r.max.y));
    let field_r = Rect::from_min_max(Pos2::new(text_left, box_r.min.y + cx.px(4.0)), Pos2::new(text_right, box_r.max.y - cx.px(3.0)));
    let show_field = open && searchable && !disabled;
    if show_field {
        let color = cx.col(cx.sch.font_color, 0.0);
        let placeholder_color = cx.col(cx.sch.placeholder(), 0.0);
        if m.open && cx.fade > 0.99 && cx.clip.intersects(box_r) && !crate::window::covered(m, cx.layer, box_r) {
            cx.request_field(TextFieldRequest {
                id,
                sub: 0,
                rect: field_r,
                text: search.clone(),
                placeholder: "Search...".to_owned(),
                font: cx.font(14.0),
                color,
                placeholder_color,
                focus: search_focus,
                numeric: false,
                max_len: None,
                clear_on_focus: false,
                password: false,
                layer: cx.layer,
            });
        } else {
            let (shown, c) = if search.is_empty() { ("Search...".to_owned(), placeholder_color) } else { (search.clone(), color) };
            let t = cx.truncate(&shown, 14.0, field_r.width(), false);
            cx.text(field_r, &t, 14.0, c, Align::Min, false, false);
        }
    } else {
        let t = cx.truncate(&display_str, 14.0, text_r.width(), false);
        cx.text(text_r, &t, 14.0, cx.col(cx.sch.font_color, text_t), Align::Min, false, false);
    }

    if expand_clicked {
        set_expanded(m, id, true);
    } else if !disabled && cx.clicked(box_r) {
        let ptr = cx.pointer().unwrap_or(Pos2::ZERO);
        let on_expand = expandable && expand_r.contains(ptr);
        let on_field = show_field && field_r.contains(ptr);
        if !on_expand && !on_field {
            let mut st = match &m.nodes[id].kind {
                Kind::Priority(p) => p.menu,
                _ => return 0.0,
            };
            menu::toggle(m, id, 0, &mut st);
            if let Kind::Priority(p) = &mut m.nodes[id].kind {
                p.menu = st;
                p.search.clear();
                p.scroll.offset = 0.0;
                p.drag = None;
            }
        }
    }

    if let Some(mr) = menu_rect {
        let full = Rect::from_min_size(mr.min, Vec2::new(mr.width(), cx.px(list_h)));
        let list = full.intersect(mr);
        let ended = cx.with_layer(Layer::Menus, mr, false, |cx| {
            menu::chrome(cx, mr, MenuCorners::Bottom);
            pass_rows(m, id, cx, View { expanded: false, list, pitch: cx.px(ROW_H), row_h: cx.px(ROW_H) }, capture)
        });
        if ended {
            fire(m, id);
        }
    }
    h
}

// ----- expanded panel -------------------------------------------------------------------------

pub(crate) fn pass_expanded(m: &mut Model, id: NodeId, cx: &mut Cx, main: Rect) {
    let (expanded, scale, active, opened_now) = match &m.nodes[id].kind {
        Kind::Priority(p) => (p.expanded, cx.anim_value(&p.expand), p.expand.active(cx.time), p.menu.opened_at == cx.time),
        _ => {
            m.active_expanded = None;
            return;
        }
    };
    if !expanded && !active {
        if m.active_expanded == Some(id) {
            m.active_expanded = None;
        }
        return;
    }
    crate::window::record_overlay(m, Layer::Window, main);
    let blocked = cx.blocked || m.active_dialog.is_some() || !m.open;
    let p = if expanded { ((scale - 0.94) / 0.06).clamp(0.0, 1.0) } else { ((scale - 0.96) / 0.04).clamp(0.0, 1.0) };
    cx.rect(main, cx.r(), cx.col(cx.sch.dark, 1.0 - 0.5 * p));

    let prev_m = cx.m;
    cx.m.s *= scale;
    let panel = Rect::from_center_size(main.center(), Vec2::new(main.width() * 0.6, main.height() * 0.68) * scale);
    let field = cx.field(id, 1).cloned();
    let focused = field.as_ref().map(|f| f.focused).unwrap_or(false);
    let capture = m.capture;
    let mut collapse = false;
    let mut ended = false;
    cx.with_layer(Layer::Window, cx.clip, blocked, |cx| {
        let (title, searchable, is_expanded) = {
            let Kind::Priority(p) = &mut m.nodes[id].kind else { return };
            if let Some(f) = &field {
                if f.text != p.expand_search {
                    p.expand_search = f.text.clone();
                    p.expand_scroll.offset = 0.0;
                }
            }
            (format!("{} (drag to rank)", p.text.as_deref().unwrap_or("Priority")), p.searchable, p.expanded)
        };
        cx.rect(panel, cx.r(), cx.col(cx.sch.background, 0.0));
        cx.outline(panel, Corners::same(cx.r()), 0.0);
        let list_top = if searchable { HEADER_H + 38.0 } else { HEADER_H };
        let list_r = Rect::from_min_max(
            Pos2::new(panel.min.x + cx.px(LIST_PAD), panel.min.y + cx.px(list_top)),
            Pos2::new(panel.max.x - cx.px(LIST_PAD), panel.max.y - cx.px(LIST_PAD)),
        );
        cx.with_clip(panel, |cx| {
            // Header.
            let header = Rect::from_min_size(panel.min, Vec2::new(panel.width(), cx.px(HEADER_H)));
            cx.hline(panel.min.x, panel.max.x, header.max.y - cx.px(1.0), cx.col(cx.sch.outline, 0.0));
            let title_r = Rect::from_min_max(Pos2::new(panel.min.x + cx.px(12.0), header.min.y), Pos2::new(panel.max.x - cx.px(56.0), header.max.y));
            let t = cx.truncate(&title, 15.0, title_r.width(), true);
            cx.text(title_r, &t, 15.0, cx.col(cx.sch.font_color, 0.0), Align::Min, false, true);
            let close_r = Rect::from_center_size(Pos2::new(panel.max.x - cx.px(8.0 + 12.0), header.center().y), Vec2::splat(cx.px(24.0)));
            if cx.hovered(close_r) {
                cx.rect(close_r, cx.r2(), cx.col(cx.sch.main, 0.0));
            }
            cx.icon(&IconRef::Lucide("x".into()), close_r.shrink(cx.px(4.0)), cx.col(cx.sch.font_color, 0.4));
            if cx.clicked(close_r) {
                collapse = true;
            }

            // Search pill.
            if searchable {
                let sr = Rect::from_min_size(Pos2::new(panel.min.x + cx.px(LIST_PAD), panel.min.y + cx.px(42.0)), Vec2::new(panel.width() - cx.px(20.0), cx.px(SEARCH_H)));
                let pill = cx.pill(sr);
                cx.rect(sr, pill, cx.col(cx.sch.main, 0.0));
                let stroke_c = if focused { cx.sch.accent } else { cx.sch.outline };
                cx.stroke(sr, pill, 1.0, cx.col(stroke_c, 0.0), StrokeKind::Inside);
                let icon_r = Rect::from_center_size(Pos2::new(sr.min.x + cx.px(10.0 + 7.5), sr.center().y), Vec2::splat(cx.px(15.0)));
                cx.icon(&IconRef::Lucide("search".into()), icon_r, cx.col(cx.sch.font_color, 0.5));
                let fr = Rect::from_min_max(Pos2::new(sr.min.x + cx.px(32.0), sr.min.y + cx.px(4.0)), Pos2::new(sr.max.x - cx.px(12.0), sr.max.y - cx.px(4.0)));
                let text = match &m.nodes[id].kind {
                    Kind::Priority(p) => p.expand_search.clone(),
                    _ => String::new(),
                };
                let color = cx.col(cx.sch.font_color, 0.0);
                let placeholder_color = cx.col(cx.sch.placeholder(), 0.0);
                if !blocked && cx.fade > 0.99 && is_expanded {
                    cx.request_field(TextFieldRequest {
                        id,
                        sub: 1,
                        rect: fr,
                        text,
                        placeholder: "Search...".to_owned(),
                        font: cx.font(14.0),
                        color,
                        placeholder_color,
                        focus: FocusRequest::None,
                        numeric: false,
                        max_len: None,
                        clear_on_focus: false,
                        password: false,
                        layer: Layer::Window,
                    });
                } else {
                    let (shown, c) = if text.is_empty() { ("Search...".to_owned(), placeholder_color) } else { (text, color) };
                    let t = cx.truncate(&shown, 14.0, fr.width(), false);
                    cx.text(fr, &t, 14.0, c, Align::Min, false, false);
                }
            }

            // Rank rows.
            ended = cx.with_clip(list_r, |cx| pass_rows(m, id, cx, View { expanded: true, list: list_r, pitch: cx.px(XROW_H), row_h: cx.px(XROW_H - 4.0) }, capture));
            let empty = match &m.nodes[id].kind {
                Kind::Priority(p) => !visibility(p, true).iter().any(|v| *v),
                _ => true,
            };
            if empty {
                let er = Rect::from_min_size(Pos2::new(panel.min.x, list_r.min.y + cx.px(4.0)), Vec2::new(panel.width(), cx.px(16.0)));
                cx.text(er, "No matching values", 14.0, cx.col(cx.sch.font_color, 0.5), Align::Center, false, false);
            }
        });

        // Click on the dimmed overlay collapses (not the click that opened the panel).
        if is_expanded && !opened_now && cx.clicked(main) {
            let ptr = cx.pointer().unwrap_or(Pos2::ZERO);
            if !panel.contains(ptr) {
                collapse = true;
            }
        }
    });
    cx.m = prev_m;
    if active {
        cx.animate();
    }
    if ended {
        fire(m, id);
    }
    if collapse {
        set_expanded(m, id, false);
    }
}
