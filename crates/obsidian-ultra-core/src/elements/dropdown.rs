//! Dropdown (single/multi, searchable, expandable, drag-select).

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::f32::consts::PI;

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use super::menu::{self, MenuCorners, MenuSpec};
use crate::draw::Corners;
use crate::frame::{Cx, FocusRequest, Layer, TextFieldRequest};
use crate::handles::info::DropdownInfo;
use crate::input::MouseButton;
use crate::node::*;
use crate::search;
use crate::tween::{info as tw, Anim};
use crate::types::*;
use crate::ui::{Model, Pending};

const ROW_H: f32 = 21.0;
const BOX_H: f32 = 21.0;
const HEADER_H: f32 = 34.0;
const CELL_H: f32 = 28.0;
const CELL_GAP: f32 = 6.0;
const LIST_PAD: f32 = 10.0;
const SEARCH_H: f32 = 26.0;

pub(crate) fn create(m: &mut Model, parent: NodeId, idx: &str, info: DropdownInfo) -> NodeId {
    // Defaults resolution: only values that exist; multi keeps all, single the first.
    // `Index` is a 0-based index into `values`.
    let candidates: Vec<String> = match &info.default {
        DropdownDefault::None => Vec::new(),
        DropdownDefault::Value(v) => vec![v.clone()],
        DropdownDefault::Values(list) => list.clone(),
        DropdownDefault::Index(i) => info.values.get(*i).map(|e| e.value.clone()).into_iter().collect(),
    };
    let resolved: Vec<String> = candidates.into_iter().filter(|v| info.values.iter().any(|e| &e.value == v)).collect();
    let value = if info.multi {
        DropdownValue::Multi(resolved.into_iter().collect())
    } else {
        DropdownValue::Single(resolved.into_iter().next())
    };
    let data = DropdownData {
        text: info.text,
        values: info.values,
        disabled_values: info.disabled_values,
        value_images: info.value_images,
        multi: info.multi,
        drag_select: info.drag_select && info.multi,
        max_visible: info.max_visible_dropdown_items.max(1),
        select_all_buttons: info.select_all_buttons,
        expandable: info.expandable,
        expand_columns: info.expand_columns.max(1),
        searchable: info.searchable,
        format_list: info.format_list_value,
        format_display: info.format_display_value,
        allow_null: info.allow_null,
        value: value.clone(),
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
        default: value,
        arrow: Default::default(),
        focus: Default::default(),
        filtered: Vec::new(),
        filter_dirty: true,
        row_hover: Vec::new(),
    };
    let id = m.insert(Kind::Dropdown(Box::new(data)), Some(parent));
    m.nodes[id].visible = info.visible;
    m.add_child(parent, id);
    m.register_option(idx, id);
    id
}

// ----- value helpers -------------------------------------------------------------------------

fn value_exists(d: &DropdownData, v: &str) -> bool {
    d.values.iter().any(|e| e.value == v)
}

/// Lua: matched against the stored value or the display text.
fn entry_disabled(d: &DropdownData, e: &DropdownEntry) -> bool {
    d.disabled_values.iter().any(|x| *x == e.value || Some(x.as_str()) == e.display.as_deref())
}

/// Row text (`FormatListValue(RawValue) or RawValue`); what search matches against.
fn list_text(d: &DropdownData, e: &DropdownEntry) -> String {
    match &d.format_list {
        Some(f) => f(e.display_text()),
        None => e.display_text().to_owned(),
    }
}

fn value_image<'a>(d: &'a DropdownData, e: &DropdownEntry) -> Option<&'a IconRef> {
    d.value_images.get(&e.value).or_else(|| e.display.as_deref().and_then(|s| d.value_images.get(s)))
}

fn use_select_all(d: &DropdownData) -> bool {
    d.multi && d.select_all_buttons
}

/// Collapsed text plus the first selected value's image.
fn display(d: &DropdownData) -> (String, Option<IconRef>) {
    let mut s = String::new();
    let mut img = None;
    match &d.value {
        DropdownValue::Multi(set) => {
            for e in &d.values {
                if !set.contains(&e.value) {
                    continue;
                }
                if !s.is_empty() {
                    s.push_str(", ");
                }
                match &d.format_display {
                    Some(f) => s.push_str(&f(e.display_text())),
                    None => s.push_str(e.display_text()),
                }
                if img.is_none() {
                    img = value_image(d, e).cloned();
                }
            }
        }
        DropdownValue::Single(Some(v)) => {
            match d.values.iter().find(|e| &e.value == v) {
                Some(e) => {
                    s = e.display_text().to_owned();
                    img = value_image(d, e).cloned();
                }
                None => s = v.clone(),
            }
            if !s.is_empty() {
                if let Some(f) = &d.format_display {
                    s = f(&s);
                }
            }
        }
        DropdownValue::Single(None) => {}
    }
    (truncate_display(&s), img)
}

/// `#Str > 25 -> sub(1,22) .. "..."`; empty -> `"---"`.
pub(crate) fn truncate_display(s: &str) -> String {
    let n = s.chars().count();
    if n == 0 {
        "---".to_owned()
    } else if n > 25 {
        let mut t: String = s.chars().take(22).collect();
        t.push_str("...");
        t
    } else {
        s.to_owned()
    }
}

/// Search filter + ordering (enabled first, score desc while searching).
fn rebuild_filtered(d: &mut DropdownData) {
    let query = search::normalize(&d.search);
    let mut entries: Vec<(usize, bool, f32)> = Vec::with_capacity(d.values.len());
    for (i, e) in d.values.iter().enumerate() {
        let score = if query.is_empty() {
            0.0
        } else {
            match search::fuzzy_score(&list_text(d, e).to_lowercase(), &query) {
                Some(s) => s,
                None => continue,
            }
        };
        entries.push((i, entry_disabled(d, e), score));
    }
    entries.sort_by(|a, b| {
        a.1.cmp(&b.1).then_with(|| if query.is_empty() { Ordering::Equal } else { b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal) })
    });
    d.filtered = entries.into_iter().map(|e| e.0).collect();
    // Two extra slots: the "Select All" / "Deselect All" header buttons.
    d.row_hover.resize(d.filtered.len() + 2, Anim::new(0.0));
    d.filter_dirty = false;
    d.scroll.offset = 0.0;
}

/// Enabled values, fuzzy-filtered by `search` when non-empty.
fn selectable_values(d: &DropdownData, search: &str) -> Vec<String> {
    let query = search::normalize(search);
    d.values
        .iter()
        .filter(|e| !entry_disabled(d, e))
        .filter(|e| query.is_empty() || search::matches(&list_text(d, e), &query))
        .map(|e| e.value.clone())
        .collect()
}

fn after_change(m: &mut Model, id: NodeId, value: DropdownValue) {
    m.update_dependency_boxes();
    m.pending.push(Pending::Dropdown(id, value));
}

/// `ToggleValue` / row click: flips `v` with the last-value guard. Returns whether the selection changed.
fn toggle_value(m: &mut Model, id: NodeId, v: &str) -> bool {
    let Some(Kind::Dropdown(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return false };
    let select = !d.value.contains(v);
    if !select && d.value.count() == 1 && !d.allow_null {
        return false;
    }
    match &mut d.value {
        DropdownValue::Multi(set) => {
            if select {
                set.insert(v.to_owned());
            } else {
                set.remove(v);
            }
        }
        DropdownValue::Single(s) => *s = if select { Some(v.to_owned()) } else { None },
    }
    let value = d.value.clone();
    after_change(m, id, value);
    true
}

// ----- public mutators -----------------------------------------------------------------------

pub(crate) fn set_value(m: &mut Model, id: NodeId, value: DropdownValue) {
    let Some(Kind::Dropdown(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    let new = match &d.value {
        DropdownValue::Multi(_) => DropdownValue::Multi(value.active_values().into_iter().filter(|v| value_exists(d, v)).collect()),
        DropdownValue::Single(cur) => match value {
            DropdownValue::Single(None) => DropdownValue::Single(None),
            DropdownValue::Single(Some(v)) => {
                if value_exists(d, &v) { DropdownValue::Single(Some(v)) } else { DropdownValue::Single(cur.clone()) }
            }
            DropdownValue::Multi(set) => match set.into_iter().find(|v| value_exists(d, v)) {
                Some(v) => DropdownValue::Single(Some(v)),
                None => DropdownValue::Single(cur.clone()),
            },
        },
    };
    d.value = new.clone();
    if !d.disabled {
        after_change(m, id, new);
    }
}

pub(crate) fn set_values(m: &mut Model, id: NodeId, values: Vec<DropdownEntry>) {
    let Some(Kind::Dropdown(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    d.values = values;
    let mut changed = false;
    let pruned = match &d.value {
        DropdownValue::Multi(set) => {
            let kept: BTreeSet<String> = set.iter().filter(|v| value_exists(d, v)).cloned().collect();
            changed = kept.len() != set.len();
            DropdownValue::Multi(kept)
        }
        DropdownValue::Single(Some(v)) if !value_exists(d, v) => {
            changed = true;
            DropdownValue::Single(None)
        }
        other => other.clone(),
    };
    d.value = pruned.clone();
    d.filter_dirty = true;
    d.drag = None;
    if changed && !d.disabled {
        after_change(m, id, pruned);
    }
}

/// Append (no callback, no display refresh).
pub(crate) fn add_values(m: &mut Model, id: NodeId, values: Vec<DropdownEntry>) {
    let Some(Kind::Dropdown(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    for v in values {
        if !value_exists(d, &v.value) {
            d.values.push(v);
        }
    }
    d.filter_dirty = true;
}

/// Select/deselect every selectable value (multi only).
pub(crate) fn bulk_select(m: &mut Model, id: NodeId, select: bool, search: &str) {
    let Some(Kind::Dropdown(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
    if !d.multi {
        return;
    }
    let list = selectable_values(d, search);
    if list.is_empty() {
        return;
    }
    let allow_null = d.allow_null;
    let DropdownValue::Multi(set) = &mut d.value else { return };
    for v in &list {
        if select {
            set.insert(v.clone());
        } else {
            set.remove(v);
        }
    }
    if !select && !allow_null && set.is_empty() {
        set.insert(list[0].clone());
    }
    let value = d.value.clone();
    after_change(m, id, value);
}

/// Collapse whichever element currently owns the expanded overlay.
pub(crate) fn collapse_other(m: &mut Model, other: NodeId) {
    match m.nodes.get(other).map(|n| &n.kind) {
        Some(Kind::Dropdown(_)) => set_expanded(m, other, false),
        Some(Kind::Priority(_)) => super::priority::set_expanded(m, other, false),
        _ => m.active_expanded = None,
    }
}

pub(crate) fn set_expanded(m: &mut Model, id: NodeId, expanded: bool) {
    let time = m.time;
    if expanded {
        match m.nodes.get(id).map(|n| &n.kind) {
            Some(Kind::Dropdown(d)) if !d.disabled && d.expandable && !d.expanded => {}
            _ => return,
        }
        if let Some(other) = m.active_expanded {
            if other != id {
                collapse_other(m, other);
            }
        }
        let mut st = match &m.nodes[id].kind {
            Kind::Dropdown(d) => d.menu,
            _ => return,
        };
        menu::close(m, id, 0, &mut st);
        let Kind::Dropdown(d) = &mut m.nodes[id].kind else { return };
        d.menu = st;
        // `opened_at` doubles as the expand time so the opening click is not read as an overlay click.
        d.menu.opened_at = time;
        d.search.clear();
        d.filter_dirty = true;
        d.expand_search.clear();
        d.expand_scroll.offset = 0.0;
        d.expanded = true;
        d.expand.play(0.94, 1.0, time, tw::DROPDOWN.0, tw::DROPDOWN.1);
        m.active_expanded = Some(id);
    } else {
        let Some(Kind::Dropdown(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) else { return };
        if !d.expanded {
            return;
        }
        d.expanded = false;
        d.expand.set(0.96, time, tw::DROPDOWN.0, tw::DROPDOWN.1);
        // `m.active_expanded` is released by `pass_expanded` once the collapse animation ends.
    }
}

pub(crate) fn set_disabled(m: &mut Model, id: NodeId, disabled: bool) {
    let mut st = match m.nodes.get(id).map(|n| &n.kind) {
        Some(Kind::Dropdown(d)) => d.menu,
        _ => return,
    };
    menu::close(m, id, 0, &mut st);
    if let Kind::Dropdown(d) = &mut m.nodes[id].kind {
        d.disabled = disabled;
        d.menu = st;
        d.drag = None;
    }
    if disabled {
        set_expanded(m, id, false);
    }
}

pub(crate) fn height(m: &Model, id: NodeId, cx: &Cx) -> f32 {
    match &m.nodes[id].kind {
        Kind::Dropdown(d) => cx.px(if d.text.is_some() { 39.0 } else { 21.0 }),
        _ => 0.0,
    }
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

    // Search text / filter upkeep.
    let (text, disabled, tips, searchable, expandable, max_visible, header, n, focus_v, search, search_focus) = {
        let Kind::Dropdown(d) = &mut m.nodes[id].kind else { return 0.0 };
        if let Some(f) = &field {
            if d.menu.open && d.searchable && f.text != d.search {
                d.search = f.text.clone();
                d.filter_dirty = true;
            }
        }
        if d.filter_dirty {
            rebuild_filtered(d);
        }
        let mut a = d.focus;
        let fv = cx.tween(&mut a, if focused && d.menu.open && !d.disabled { 1.0 } else { 0.0 }, tw::HOVER);
        d.focus = a;
        let header = if use_select_all(d) { ROW_H } else { 0.0 };
        (
            d.text.clone(),
            d.disabled,
            d.tips.clone(),
            d.searchable,
            d.expandable,
            d.max_visible,
            header,
            d.filtered.len(),
            fv,
            d.search.clone(),
            std::mem::replace(&mut d.search_focus, FocusRequest::None),
        )
    };

    // Inline list menu.
    let list_h = (n.min(max_visible) as f32) * ROW_H + header;
    let spec = MenuSpec {
        holder: box_r,
        width: box_r.width(),
        offset: Vec2::new(cx.px(0.5), cx.px(BOX_H + 1.5)),
        height: cx.px(list_h),
        anim: if animate_menu { Some(tw::DROPDOWN) } else { None },
    };
    let mut st = match &m.nodes[id].kind {
        Kind::Dropdown(d) => d.menu,
        _ => return 0.0,
    };
    let menu_rect = menu::begin(m, id, 0, cx, &mut st, spec);
    let open = st.open;
    let arrow_v = {
        let Kind::Dropdown(d) = &mut m.nodes[id].kind else { return 0.0 };
        d.menu = st;
        if !open && !d.search.is_empty() {
            d.search.clear();
            d.filter_dirty = true;
        }
        let mut a = d.arrow;
        let v = cx.tween(&mut a, if open { 1.0 } else { 0.0 }, tw::HOVER);
        d.arrow = a;
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

    // Chevron (rotates 180° while open).
    let arrow_r = Rect::from_center_size(Pos2::new(box_r.max.x - cx.px(4.0 + 8.0), box_r.center().y), Vec2::splat(cx.px(16.0)));
    let arrow_t = if disabled { 0.8 } else { 0.5 * (1.0 - arrow_v) };
    cx.icon_rotated(&IconRef::Lucide("chevron-up".into()), arrow_r, cx.col(cx.sch.font_color, arrow_t), PI * arrow_v);

    // Expand button.
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

    // Display text / value image / inline search field.
    let (display_str, image) = match &m.nodes[id].kind {
        Kind::Dropdown(d) => display(d),
        _ => return 0.0,
    };
    let mut text_left = box_r.min.x + cx.px(8.0);
    if let Some(img) = &image {
        let ir = Rect::from_min_size(Pos2::new(box_r.min.x + cx.px(4.0), box_r.min.y + cx.px(3.0)), Vec2::splat(cx.px(16.0)));
        cx.icon(img, ir, cx.col(cx.sch.font_color, text_t));
        text_left += cx.px(14.0);
    }
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

    // Open/close on box click (the expand button and a live search field swallow their clicks).
    if expand_clicked {
        set_expanded(m, id, true);
    } else if !disabled && cx.clicked(box_r) {
        let p = cx.pointer().unwrap_or(Pos2::ZERO);
        let on_expand = expandable && expand_r.contains(p);
        let on_field = show_field && field_r.contains(p);
        if !on_expand && !on_field {
            let mut st = match &m.nodes[id].kind {
                Kind::Dropdown(d) => d.menu,
                _ => return 0.0,
            };
            menu::toggle(m, id, 0, &mut st);
            if let Kind::Dropdown(d) = &mut m.nodes[id].kind {
                d.menu = st;
                d.search.clear();
                d.filter_dirty = true;
                d.scroll.offset = 0.0;
                d.drag = None;
            }
        }
    }

    if let Some(mr) = menu_rect {
        pass_menu(m, id, cx, mr, cx.px(list_h), cx.px(header));
    }
    h
}

enum MenuAction {
    Bulk(bool),
    Toggle(String),
    DragEnd,
}

fn pass_menu(m: &mut Model, id: NodeId, cx: &mut Cx, mr: Rect, full_h: f32, header_h: f32) {
    let capture = m.capture;
    let captured = capture.map(|c| c.node == id && c.sub == cap::DRAG_SELECT).unwrap_or(false);
    let mut actions: Vec<MenuAction> = Vec::new();
    let mut start_drag = false;
    cx.with_layer(Layer::Menus, mr, false, |cx| {
        menu::chrome(cx, mr, MenuCorners::Bottom);
        let full = Rect::from_min_size(mr.min, Vec2::new(mr.width(), full_h));
        let list_r = Rect::from_min_max(Pos2::new(full.min.x, full.min.y + header_h), full.max);
        let row_h = cx.px(ROW_H);
        let wheel = cx.wheel_over(mr);
        let Kind::Dropdown(d) = &mut m.nodes[id].kind else { return };
        let n = d.filtered.len();
        d.scroll.view = list_r.height();
        d.scroll.content = n as f32 * row_h;
        if wheel != 0.0 && capture.is_none() {
            d.scroll.wheel(wheel);
        }
        d.scroll.clamp();

        // Select All / Deselect All header.
        if header_h > 0.0 {
            let hr = Rect::from_min_size(full.min, Vec2::new(full.width(), header_h));
            let half = hr.width() / 2.0;
            let buttons = [
                (Rect::from_min_size(hr.min, Vec2::new(half, hr.height())), "Select All", true),
                (Rect::from_min_size(Pos2::new(hr.min.x + half, hr.min.y), Vec2::new(half, hr.height())), "Deselect All", false),
            ];
            for (k, (br, label, state)) in buttons.into_iter().enumerate() {
                let hovered = cx.hovered(br);
                let mut a = d.row_hover[n + k];
                let hv = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
                d.row_hover[n + k] = a;
                cx.text(br, label, 14.0, cx.col(cx.sch.font_color, 0.5 * (1.0 - hv)), Align::Center, false, false);
                if cx.clicked(br) {
                    actions.push(MenuAction::Bulk(state));
                }
            }
            cx.hline(hr.min.x, hr.max.x, hr.max.y - cx.px(1.0), cx.col(cx.sch.outline, 0.0));
        }

        // Pooled rows: only the visible window of `filtered` is laid out.
        let first = ((d.scroll.offset / row_h).floor().max(0.0) as usize).min(n.saturating_sub(1));
        let last = (first + d.max_visible + 2).min(n);
        let mut hover_row: Option<usize> = None;
        let mut press_row: Option<usize> = None;
        let mut click_row: Option<usize> = None;
        for i in first..last {
            let vi = d.filtered[i];
            let row_r = Rect::from_min_size(Pos2::new(list_r.min.x, list_r.min.y + i as f32 * row_h - d.scroll.offset), Vec2::new(list_r.width(), row_h));
            if !row_r.intersects(mr) {
                continue;
            }
            let e = &d.values[vi];
            let selected = d.value.contains(&e.value);
            let dis = entry_disabled(d, e);
            let over = cx.hovered(row_r);
            if over {
                hover_row = Some(i);
            }
            let mut a = d.row_hover[i];
            let hv = cx.tween(&mut a, if over && !dis && !selected { 1.0 } else { 0.0 }, tw::HOVER);
            d.row_hover[i] = a;
            let e = &d.values[vi];
            let (bg_t, text_t) = if selected {
                (0.0, 0.0)
            } else if dis {
                (1.0, 0.8)
            } else {
                (1.0 - 0.15 * hv, 0.5 - 0.25 * hv)
            };
            let corners = if i + 1 == n { Corners::bottom(cx.r2()) } else { Corners::ZERO };
            if bg_t < 0.999 {
                cx.rect(row_r, corners, cx.col(cx.sch.main, bg_t));
            }
            let mut tx = row_r.min.x + cx.px(7.0);
            if let Some(img) = value_image(d, e) {
                let ir = Rect::from_min_size(Pos2::new(row_r.min.x + cx.px(4.0), row_r.min.y + cx.px(3.0)), Vec2::splat(cx.px(16.0)));
                cx.icon(img, ir, cx.col(cx.sch.font_color, text_t));
                tx += cx.px(18.0);
            }
            let text_r = Rect::from_min_max(Pos2::new(tx, row_r.min.y), Pos2::new(row_r.max.x - cx.px(7.0), row_r.max.y));
            let label = list_text(d, e);
            let t = cx.truncate(&label, 14.0, text_r.width(), false);
            cx.text(text_r, &t, 14.0, cx.col(cx.sch.font_color, text_t), Align::Min, false, false);
            if over && !dis && cx.pressed(MouseButton::Left) {
                press_row = Some(i);
                click_row = Some(i);
            }
        }

        // Scrollbar (2px).
        if d.scroll.scrollable() {
            let track = Rect::from_min_max(Pos2::new(list_r.max.x - cx.px(2.0), list_r.min.y), list_r.max);
            let thumb = d.scroll.thumb(track);
            cx.rect(thumb, cx.pill(thumb), cx.col(cx.sch.outline, 0.0));
        }

        // Drag-select.
        if d.drag.is_some() {
            if captured {
                if let Some(i) = hover_row {
                    update_drag(d, i);
                }
                cx.animate();
            } else {
                d.drag = None;
                actions.push(MenuAction::DragEnd);
            }
        } else if let Some(i) = press_row {
            if d.drag_select && d.multi && capture.is_none() {
                let mut initial = HashMap::with_capacity(d.filtered.len());
                for &vi in &d.filtered {
                    let v = &d.values[vi].value;
                    initial.insert(v.clone(), d.value.contains(v));
                }
                d.drag = Some(DragSelect { start: i, last_range: None, initial, active_count: d.value.count() });
                update_drag(d, i);
                start_drag = true;
            } else if let Some(i) = click_row {
                actions.push(MenuAction::Toggle(d.values[d.filtered[i]].value.clone()));
            }
        }
    });
    if start_drag {
        m.capture = Some(Capture { node: id, sub: cap::DRAG_SELECT, start: cx.pointer().unwrap_or(Pos2::ZERO), data: [0.0; 4], since: cx.time });
    }
    for a in actions {
        match a {
            MenuAction::Bulk(state) => {
                let search = match &m.nodes[id].kind {
                    Kind::Dropdown(d) => d.search.clone(),
                    _ => String::new(),
                };
                bulk_select(m, id, state, &search);
            }
            MenuAction::Toggle(v) => {
                toggle_value(m, id, &v);
            }
            MenuAction::DragEnd => {
                let value = match &m.nodes[id].kind {
                    Kind::Dropdown(d) => d.value.clone(),
                    _ => continue,
                };
                after_change(m, id, value);
            }
        }
    }
}

/// Apply the range between the start row and `idx` as a diff against the previous range.
fn update_drag(d: &mut DropdownData, idx: usize) {
    let Some(mut drag) = d.drag.take() else { return };
    let (lo, hi) = (drag.start.min(idx), drag.start.max(idx));
    match drag.last_range {
        None => {
            for i in lo..=hi {
                apply_drag_index(d, &mut drag, i, true);
            }
        }
        Some((plo, phi)) if (plo, phi) != (lo, hi) => {
            for i in plo..=phi {
                if i < lo || i > hi {
                    apply_drag_index(d, &mut drag, i, false);
                }
            }
            for i in lo..=hi {
                if i < plo || i > phi {
                    apply_drag_index(d, &mut drag, i, true);
                }
            }
        }
        Some(_) => {}
    }
    drag.last_range = Some((lo, hi));
    d.drag = Some(drag);
}

fn apply_drag_index(d: &mut DropdownData, drag: &mut DragSelect, i: usize, in_range: bool) {
    let Some(&vi) = d.filtered.get(i) else { return };
    let e = &d.values[vi];
    if entry_disabled(d, e) {
        return;
    }
    let v = e.value.clone();
    let initial = drag.initial.get(&v).copied().unwrap_or(false);
    let want = if in_range { !initial } else { initial };
    let allow_null = d.allow_null;
    let DropdownValue::Multi(set) = &mut d.value else { return };
    if set.contains(&v) == want {
        return;
    }
    if !want && drag.active_count == 1 && !allow_null {
        return;
    }
    if want {
        set.insert(v);
        drag.active_count += 1;
    } else {
        set.remove(&v);
        drag.active_count = drag.active_count.saturating_sub(1);
    }
}

// ----- expanded panel -------------------------------------------------------------------------

enum Cell {
    Bulk(bool),
    Value(usize),
}

enum PanelAction {
    Collapse,
    Bulk(bool),
    Toggle(String),
}

pub(crate) fn pass_expanded(m: &mut Model, id: NodeId, cx: &mut Cx, main: Rect) {
    let (expanded, scale, active, opened_now) = match &m.nodes[id].kind {
        Kind::Dropdown(d) => (d.expanded, cx.anim_value(&d.expand), d.expand.active(cx.time), d.menu.opened_at == cx.time),
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

    // The panel scales around its center (Roblox `UIScale`): scale every design unit.
    let prev_m = cx.m;
    cx.m.s *= scale;
    let panel = Rect::from_center_size(main.center(), Vec2::new(main.width() * 0.7, main.height() * 0.72) * scale);
    let field = cx.field(id, 1).cloned();
    let focused = field.as_ref().map(|f| f.focused).unwrap_or(false);
    let mut actions: Vec<PanelAction> = Vec::new();
    cx.with_layer(Layer::Window, cx.clip, blocked, |cx| {
        let Kind::Dropdown(d) = &mut m.nodes[id].kind else { return };
        if let Some(f) = &field {
            if f.text != d.expand_search {
                d.expand_search = f.text.clone();
                d.expand_scroll.offset = 0.0;
            }
        }
        cx.rect(panel, cx.r(), cx.col(cx.sch.background, 0.0));
        cx.outline(panel, Corners::same(cx.r()), 0.0);
        cx.with_clip(panel, |cx| {
            // Header.
            let header = Rect::from_min_size(panel.min, Vec2::new(panel.width(), cx.px(HEADER_H)));
            cx.hline(panel.min.x, panel.max.x, header.max.y - cx.px(1.0), cx.col(cx.sch.outline, 0.0));
            let title_r = Rect::from_min_max(Pos2::new(panel.min.x + cx.px(12.0), header.min.y), Pos2::new(panel.max.x - cx.px(56.0), header.max.y));
            let title = d.text.clone().unwrap_or_else(|| "Select a value".to_owned());
            let title = cx.truncate(&title, 15.0, title_r.width(), true);
            cx.text(title_r, &title, 15.0, cx.col(cx.sch.font_color, 0.0), Align::Min, false, true);
            let close_r = Rect::from_center_size(Pos2::new(panel.max.x - cx.px(8.0 + 11.0), header.center().y), Vec2::splat(cx.px(22.0)));
            if cx.hovered(close_r) {
                cx.rect(close_r, cx.r2(), cx.col(cx.sch.main, 0.0));
            }
            cx.icon(&IconRef::Lucide("x".into()), close_r.shrink(cx.px(4.0)), cx.col(cx.sch.font_color, 0.4));
            if cx.clicked(close_r) {
                actions.push(PanelAction::Collapse);
            }

            // Search pill.
            let list_top = if d.searchable { HEADER_H + 38.0 } else { HEADER_H };
            if d.searchable {
                let sr = Rect::from_min_size(Pos2::new(panel.min.x + cx.px(LIST_PAD), panel.min.y + cx.px(42.0)), Vec2::new(panel.width() - cx.px(20.0), cx.px(SEARCH_H)));
                let pill = cx.pill(sr);
                cx.rect(sr, pill, cx.col(cx.sch.main, 0.0));
                let stroke_c = if focused { cx.sch.accent } else { cx.sch.outline };
                cx.stroke(sr, pill, 1.0, cx.col(stroke_c, 0.0), StrokeKind::Inside);
                let icon_r = Rect::from_center_size(Pos2::new(sr.min.x + cx.px(10.0 + 7.5), sr.center().y), Vec2::splat(cx.px(15.0)));
                cx.icon(&IconRef::Lucide("search".into()), icon_r, cx.col(cx.sch.font_color, 0.5));
                let fr = Rect::from_min_max(Pos2::new(sr.min.x + cx.px(32.0), sr.min.y + cx.px(4.0)), Pos2::new(sr.max.x - cx.px(12.0), sr.max.y - cx.px(4.0)));
                let color = cx.col(cx.sch.font_color, 0.0);
                let placeholder_color = cx.col(cx.sch.placeholder(), 0.0);
                if !blocked && cx.fade > 0.99 && d.expanded {
                    cx.request_field(TextFieldRequest {
                        id,
                        sub: 1,
                        rect: fr,
                        text: d.expand_search.clone(),
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
                    let (shown, c) = if d.expand_search.is_empty() { ("Search...".to_owned(), placeholder_color) } else { (d.expand_search.clone(), color) };
                    let t = cx.truncate(&shown, 14.0, fr.width(), false);
                    cx.text(fr, &t, 14.0, c, Align::Min, false, false);
                }
            }

            // Grid.
            let list_r = Rect::from_min_max(Pos2::new(panel.min.x, panel.min.y + cx.px(list_top)), panel.max);
            let query = search::normalize(&d.expand_search);
            let mut cells: Vec<(Cell, bool)> = Vec::with_capacity(d.values.len() + 2);
            if use_select_all(d) {
                cells.push((Cell::Bulk(true), false));
                cells.push((Cell::Bulk(false), false));
            }
            for (i, e) in d.values.iter().enumerate() {
                if !query.is_empty() && !search::matches(&list_text(d, e), &query) {
                    continue;
                }
                cells.push((Cell::Value(i), entry_disabled(d, e)));
            }
            cells.sort_by_key(|c| c.1);
            let cols = d.expand_columns.max(1);
            let pad = cx.px(LIST_PAD);
            let gap = cx.px(CELL_GAP);
            let cell_h = cx.px(CELL_H);
            let cell_w = ((list_r.width() - 2.0 * pad - gap * (cols as f32 - 1.0)) / cols as f32).max(1.0);
            let rows = cells.len().div_ceil(cols);
            d.expand_scroll.view = list_r.height();
            d.expand_scroll.content = if rows == 0 { 0.0 } else { rows as f32 * cell_h + (rows as f32 - 1.0) * gap + 2.0 * pad };
            let wheel = cx.wheel_over(list_r);
            if wheel != 0.0 {
                d.expand_scroll.wheel(wheel);
            }
            d.expand_scroll.clamp();
            let offset = d.expand_scroll.offset;
            if cells.is_empty() {
                let er = Rect::from_min_size(Pos2::new(panel.min.x, list_r.min.y + cx.px(14.0)), Vec2::new(panel.width(), cx.px(16.0)));
                cx.text(er, "No matching values", 14.0, cx.col(cx.sch.font_color, 0.5), Align::Center, false, false);
            }
            cx.with_clip(list_r, |cx| {
                for (k, (cell, dis)) in cells.iter().enumerate() {
                    let col = (k % cols) as f32;
                    let row = (k / cols) as f32;
                    let cr = Rect::from_min_size(
                        Pos2::new(list_r.min.x + pad + col * (cell_w + gap), list_r.min.y + pad + row * (cell_h + gap) - offset),
                        Vec2::new(cell_w, cell_h),
                    );
                    if !cr.intersects(list_r) {
                        continue;
                    }
                    let hovered = cx.hovered(cr);
                    match cell {
                        Cell::Bulk(state) => {
                            cx.rect(cr, cx.r2(), cx.col(cx.sch.main, if hovered { 0.5 } else { 1.0 }));
                            cx.stroke(cr, cx.r2(), 1.0, cx.col(cx.sch.outline, 0.5), StrokeKind::Inside);
                            let label = if *state { "Select All" } else { "Deselect All" };
                            cx.text(cr, label, 14.0, cx.col(cx.sch.font_color, 0.4), Align::Center, false, false);
                            if cx.clicked(cr) {
                                actions.push(PanelAction::Bulk(*state));
                            }
                        }
                        Cell::Value(vi) => {
                            let e = &d.values[*vi];
                            let selected = d.value.contains(&e.value);
                            let (bg_t, stroke_t, text_t) = if selected {
                                (0.0, 0.2, 0.0)
                            } else if *dis {
                                (1.0, 0.7, 0.8)
                            } else {
                                (if hovered { 0.5 } else { 1.0 }, 0.7, 0.4)
                            };
                            cx.rect(cr, cx.r2(), cx.col(cx.sch.main, bg_t));
                            cx.stroke(cr, cx.r2(), 1.0, cx.col(cx.sch.outline, stroke_t), StrokeKind::Inside);
                            let mut tx = cr.min.x + cx.px(10.0);
                            if let Some(img) = value_image(d, e) {
                                let ir = Rect::from_center_size(Pos2::new(cr.min.x + cx.px(8.0 + 9.0), cr.center().y), Vec2::splat(cx.px(18.0)));
                                cx.icon(img, ir, cx.col(cx.sch.font_color, text_t));
                                tx = cr.min.x + cx.px(30.0);
                            }
                            let tr = Rect::from_min_max(Pos2::new(tx, cr.min.y), Pos2::new(cr.max.x - cx.px(10.0), cr.max.y));
                            let label = list_text(d, e);
                            let t = cx.truncate(&label, 14.0, tr.width(), false);
                            cx.text(tr, &t, 14.0, cx.col(cx.sch.font_color, text_t), Align::Min, false, false);
                            if !*dis && cx.clicked(cr) {
                                actions.push(PanelAction::Toggle(e.value.clone()));
                            }
                        }
                    }
                }
            });
            if d.expand_scroll.scrollable() {
                let track = Rect::from_min_max(Pos2::new(list_r.max.x - cx.px(2.0), list_r.min.y), list_r.max);
                let thumb = d.expand_scroll.thumb(track);
                cx.rect(thumb, cx.pill(thumb), cx.col(cx.sch.outline, 0.0));
            }
        });

        // Click on the dimmed overlay collapses (not the click that opened the panel).
        if d.expanded && !opened_now && cx.clicked(main) {
            let p = cx.pointer().unwrap_or(Pos2::ZERO);
            if !panel.contains(p) {
                actions.push(PanelAction::Collapse);
            }
        }
    });
    cx.m = prev_m;
    if active {
        cx.animate();
    }

    let (multi, expand_search) = match &m.nodes[id].kind {
        Kind::Dropdown(d) => (d.multi, d.expand_search.clone()),
        _ => return,
    };
    for a in actions {
        match a {
            PanelAction::Collapse => set_expanded(m, id, false),
            PanelAction::Bulk(state) => bulk_select(m, id, state, &expand_search),
            PanelAction::Toggle(v) => {
                toggle_value(m, id, &v);
                if !multi {
                    set_expanded(m, id, false);
                }
            }
        }
    }
}
