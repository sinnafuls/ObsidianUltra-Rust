//! Sidebar tabs, key tabs, sub-tabs, columns, warning box, banners, tab-info header.

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, FocusRequest, Layer, TextFieldRequest};
use crate::handles::info::{KeyTabInfo, SubTabInfo, TabInfo};
use crate::input::MouseButton;
use crate::layout::Scroll;
use crate::node::*;
use crate::tween::{info as tw, Anim, Ease};
use crate::types::*;
use crate::ui::Model;

pub(crate) const TAB_BUTTON_H: f32 = 40.0;
const SUBTAB_BAR_H: f32 = 32.0;
const SUBTAB_IDLE: f32 = 0.4;
const SUBTAB_ICON: f32 = 16.0;
const SUBTAB_UNDERLINE_W: f32 = 0.66;
const SUBTAB_UNDERLINE_GAP: f32 = 3.0;
const SUBTAB_SHADOW: [f32; 2] = [0.55, 0.75];
const SUBTAB_HOVER_SCALE: f32 = 0.94;
const SIDEBAR_INDENT: f32 = 30.0;
const SIDEBAR_ICON_COL: f32 = 20.0;

pub(crate) fn create_tab(m: &mut Model, win: NodeId, info: TabInfo) -> NodeId {
    let order = info.order.unwrap_or_else(|| match &m.nodes[win].kind { Kind::Window(w) => w.tabs.len() as i32, _ => 0 });
    let name = if info.name.is_empty() { "Tab".to_owned() } else { info.name };
    let data = TabData {
        tips: Tips { tooltip: info.tooltip.or_else(|| Some(name.clone())), disabled_tooltip: info.disabled_tooltip },
        name,
        description: info.description,
        icon: info.icon,
        order,
        layout: info.layout,
        boxes: Vec::new(),
        sub_tabs: Vec::new(),
        active_sub_tab: None,
        sidebar_expanded: false,
        sidebar_list: Anim::new(0.0),
        chevron: Anim::new(0.0),
        warning: WarningBox::default(),
        banners: Vec::new(),
        sub_tab_align: Align_::Center,
        underline_x: Anim::new(0.0),
        underline_w: Anim::new(0.0),
        underline_visible: false,
        scroll: [Scroll::default(); 2],
        canvas: Anim::new(0.0),
        active: Anim::new(0.0),
    };
    let id = m.insert(Kind::Tab(Box::new(data)), Some(win));
    if let Kind::Window(w) = &mut m.nodes[win].kind {
        w.tabs.push(id);
    }
    sort_tabs(m);
    let active = match &m.nodes[win].kind { Kind::Window(w) => w.active_tab, _ => None };
    if active.is_none() {
        show(m, id);
    }
    id
}

type Align_ = crate::types::Align;

pub(crate) fn create_key_tab(m: &mut Model, win: NodeId, info: KeyTabInfo) -> NodeId {
    let order = info.order.unwrap_or_else(|| match &m.nodes[win].kind { Kind::Window(w) => w.tabs.len() as i32, _ => 0 });
    let data = KeyTabData { name: if info.name.is_empty() { "Tab".to_owned() } else { info.name }, description: info.description, icon: info.icon, order, scroll: Scroll::default(), canvas: Anim::new(0.0), active: Anim::new(0.0) };
    let id = m.insert(Kind::KeyTab(data), Some(win));
    if let Kind::Window(w) = &mut m.nodes[win].kind {
        w.tabs.push(id);
    }
    sort_tabs(m);
    let active = match &m.nodes[win].kind { Kind::Window(w) => w.active_tab, _ => None };
    if active.is_none() {
        show(m, id);
    }
    id
}

pub(crate) fn create_sub_tab(m: &mut Model, tab: NodeId, info: SubTabInfo) -> NodeId {
    let data = SubTabData {
        name: if info.name.is_empty() { "SubTab".to_owned() } else { info.name },
        icon: info.icon,
        boxes: Vec::new(),
        scroll: [Scroll::default(); 2],
        canvas: Anim::new(0.0),
        active: Anim::new(0.0),
        chip: Anim::new(0.0),
        scale: Anim::new(1.0),
        entry_active: Anim::new(0.0),
        entry_hover: Anim::new(0.0),
    };
    let id = m.insert(Kind::SubTab(Box::new(data)), Some(tab));
    let first = if let Kind::Tab(t) = &mut m.nodes[tab].kind {
        t.sub_tabs.push(id);
        t.active_sub_tab.is_none()
    } else {
        false
    };
    let is_active_tab = m.window.and_then(|w| match &m.nodes[w].kind { Kind::Window(wd) => wd.active_tab, _ => None }) == Some(tab);
    if let Kind::Tab(t) = &mut m.nodes[tab].kind {
        if is_active_tab {
            t.sidebar_expanded = true;
        }
    }
    if first {
        show(m, id);
    }
    id
}

pub(crate) fn sort_tabs(m: &mut Model) {
    let Some(win) = m.window else { return };
    let mut tabs = match &m.nodes[win].kind { Kind::Window(w) => w.tabs.clone(), _ => return };
    tabs.sort_by_key(|id| match &m.nodes[*id].kind {
        Kind::Tab(t) => t.order,
        Kind::KeyTab(t) => t.order,
        _ => 0,
    });
    if let Kind::Window(w) = &mut m.nodes[win].kind {
        w.tabs = tabs;
    }
}

/// `Tab:Show` / `SubTab:Show` / `KeyTab:Show`.
pub(crate) fn show(m: &mut Model, id: NodeId) {
    if !m.alive(id) {
        return;
    }
    let time = m.time;
    let animate = m.settings.animations.tab_switch;
    let transition = m.window.map(|w| match &m.nodes[w].kind { Kind::Window(wd) => wd.tab_transition_time, _ => 0.22 }).unwrap_or(0.22);
    match &m.nodes[id].kind {
        Kind::Tab(_) | Kind::KeyTab(_) => {
            let Some(win) = m.window else { return };
            let prev = match &m.nodes[win].kind { Kind::Window(w) => w.active_tab, _ => None };
            if prev == Some(id) {
                return;
            }
            if let Some(p) = prev {
                hide(m, p);
            }
            if let Kind::Window(w) = &mut m.nodes[win].kind {
                w.active_tab = Some(id);
            }
            match &mut m.nodes[id].kind {
                Kind::Tab(t) => {
                    t.active.set(1.0, time, tw::HOVER.0, tw::HOVER.1);
                    if !t.sub_tabs.is_empty() {
                        t.sidebar_expanded = true;
                    }
                    if animate {
                        t.canvas.play(0.0, 1.0, time, transition, Ease::QuadOut);
                    } else {
                        t.canvas.snap(1.0);
                    }
                }
                Kind::KeyTab(t) => {
                    t.active.set(1.0, time, tw::HOVER.0, tw::HOVER.1);
                    if animate {
                        t.canvas.play(0.0, 1.0, time, transition, Ease::QuadOut);
                    } else {
                        t.canvas.snap(1.0);
                    }
                }
                _ => {}
            }
            if m.searching {
                let t = m.search_text.clone();
                crate::overlays::search::update_search(m, &t);
            }
        }
        Kind::SubTab(_) => {
            let Some(tab) = m.nodes[id].parent else { return };
            let prev = match &m.nodes[tab].kind { Kind::Tab(t) => t.active_sub_tab, _ => None };
            if prev == Some(id) {
                return;
            }
            if let Some(p) = prev {
                if let Kind::SubTab(st) = &mut m.nodes[p].kind {
                    st.active.set(0.0, time, tw::HOVER.0, tw::HOVER.1);
                    st.entry_active.set(0.0, time, tw::HOVER.0, tw::HOVER.1);
                    st.canvas.snap(0.0);
                }
            }
            if let Kind::Tab(t) = &mut m.nodes[tab].kind {
                t.active_sub_tab = Some(id);
            }
            if let Kind::SubTab(st) = &mut m.nodes[id].kind {
                st.active.set(1.0, time, tw::HOVER.0, tw::HOVER.1);
                st.entry_active.set(1.0, time, tw::HOVER.0, tw::HOVER.1);
                if animate {
                    st.canvas.play(0.0, 1.0, time, transition, Ease::QuadOut);
                } else {
                    st.canvas.snap(1.0);
                }
            }
            if m.searching {
                let t = m.search_text.clone();
                crate::overlays::search::update_search(m, &t);
            }
        }
        _ => {}
    }
}

/// `Tab:Hide` / `SubTab:Hide` / `KeyTab:Hide`.
pub(crate) fn hide(m: &mut Model, id: NodeId) {
    if !m.alive(id) {
        return;
    }
    let time = m.time;
    match &m.nodes[id].kind {
        Kind::Tab(_) | Kind::KeyTab(_) => {
            let Some(win) = m.window else { return };
            if let Kind::Window(w) = &mut m.nodes[win].kind {
                if w.active_tab == Some(id) {
                    w.active_tab = None;
                }
            }
            match &mut m.nodes[id].kind {
                Kind::Tab(t) => {
                    t.active.set(0.0, time, tw::HOVER.0, tw::HOVER.1);
                    t.sidebar_expanded = false;
                    t.canvas.snap(0.0);
                }
                Kind::KeyTab(t) => {
                    t.active.set(0.0, time, tw::HOVER.0, tw::HOVER.1);
                    t.canvas.snap(0.0);
                }
                _ => {}
            }
        }
        Kind::SubTab(_) => {
            let Some(tab) = m.nodes[id].parent else { return };
            if let Kind::Tab(t) = &mut m.nodes[tab].kind {
                if t.active_sub_tab == Some(id) {
                    t.active_sub_tab = None;
                    t.underline_visible = false;
                }
            }
            if let Kind::SubTab(st) = &mut m.nodes[id].kind {
                st.active.set(0.0, time, tw::HOVER.0, tw::HOVER.1);
                st.entry_active.set(0.0, time, tw::HOVER.0, tw::HOVER.1);
                st.canvas.snap(0.0);
            }
        }
        _ => {}
    }
}

/// A sub tab became hidden/destroyed: move to the first other visible one.
pub(crate) fn subtab_hidden(m: &mut Model, id: NodeId) {
    let Some(tab) = m.nodes[id].parent else { return };
    let (active, subs) = match &m.nodes[tab].kind { Kind::Tab(t) => (t.active_sub_tab, t.sub_tabs.clone()), _ => return };
    if active != Some(id) {
        return;
    }
    hide(m, id);
    if let Some(next) = subs.into_iter().find(|s| *s != id && m.nodes[*s].shown()) {
        show(m, next);
    }
}

// ----- tab info header --------------------------------------------------------------------

pub(crate) fn pass_tab_info(m: &mut Model, win: NodeId, cx: &mut Cx, rect: Rect) {
    let active = match &m.nodes[win].kind { Kind::Window(w) => w.active_tab, _ => None };
    let Some(t) = active else { return };
    let (name, desc) = match &m.nodes[t].kind {
        Kind::Tab(td) => (td.name.clone(), td.description.clone().unwrap_or_default()),
        Kind::KeyTab(td) => (td.name.clone(), td.description.clone().unwrap_or_default()),
        _ => return,
    };
    let inner = Rect::from_min_max(Pos2::new(rect.min.x + cx.px(12.0), rect.min.y), Pos2::new(rect.max.x - cx.px(12.0), rect.max.y));
    if inner.width() <= 0.0 {
        return;
    }
    let font_c = cx.col(cx.sch.font_color, 0.0);
    let name_h = cx.text_size(&name, 17.0, None, true).y;
    let desc = if desc.is_empty() { desc } else { cx.truncate(&desc, 14.0, inner.width(), true) };
    let desc_h = if desc.is_empty() { 0.0 } else { cx.text_size(&desc, 14.0, None, true).y };
    let total = name_h + desc_h;
    let y = inner.center().y - total / 2.0;
    cx.with_clip(inner, |cx| {
        let nr = Rect::from_min_size(Pos2::new(inner.min.x, y), Vec2::new(inner.width(), name_h));
        cx.text(nr, &format!("<b>{name}</b>"), 17.0, font_c, Align::Min, false, true);
        if desc_h > 0.0 {
            let dr = Rect::from_min_size(Pos2::new(inner.min.x, y + name_h), Vec2::new(inner.width(), desc_h));
            cx.text_top(dr, &desc, 14.0, cx.col(cx.sch.font_color, 0.45), Align::Min, false);
        }
    });
}

// ----- sidebar ----------------------------------------------------------------------------

pub(crate) fn pass_sidebar(m: &mut Model, win: NodeId, cx: &mut Cx, rect: Rect) {
    let (tabs, compact, active) = match &m.nodes[win].kind { Kind::Window(w) => (w.tabs.clone(), w.compact, w.active_tab), _ => return };
    let mut y = rect.min.y;
    for id in tabs {
        if !m.nodes[id].shown() {
            continue;
        }
        let is_active = active == Some(id);
        let h = pass_tab_button(m, id, cx, Pos2::new(rect.min.x, y), rect.width(), compact, is_active);
        y += h;
    }
}

fn pass_tab_button(m: &mut Model, id: NodeId, cx: &mut Cx, pos: Pos2, w: f32, compact: bool, is_active: bool) -> f32 {
    let bh = cx.px(TAB_BUTTON_H);
    let r = Rect::from_min_size(pos, Vec2::new(w, bh));
    m.nodes[id].rect = r;
    let hovered = cx.hovered(r);
    let (name, icon, has_subs, tooltip, expanded, subs) = match &m.nodes[id].kind {
        Kind::Tab(t) => (t.name.clone(), t.icon.clone(), !t.sub_tabs.is_empty(), t.tips.tooltip.clone(), t.sidebar_expanded, t.sub_tabs.clone()),
        Kind::KeyTab(t) => (t.name.clone(), t.icon.clone(), false, None, false, Vec::new()),
        _ => return 0.0,
    };
    let active_v = match &mut m.nodes[id].kind {
        Kind::Tab(t) => cx.anim_value(&t.active),
        Kind::KeyTab(t) => cx.anim_value(&t.active),
        _ => 0.0,
    };
    let hover_v = {
        let n = &mut m.nodes[id];
        let mut a = n.hover;
        let v = cx.tween(&mut a, if hovered && !is_active { 1.0 } else { 0.0 }, tw::HOVER);
        n.hover = a;
        v
    };
    // Background (active).
    cx.rect(r, 0.0, cx.col(cx.sch.main, 1.0 - active_v));
    let transparency = 0.5 - 0.25 * hover_v - 0.5 * active_v;
    let (pad_x, pad_y) = if compact { (6.0, 6.0) } else { (12.0, 11.0) };
    let content = Rect::from_min_max(Pos2::new(r.min.x + cx.px(pad_x), r.min.y + cx.px(pad_y)), Pos2::new(r.max.x - cx.px(pad_x), r.max.y - cx.px(pad_y)));
    let font_c = cx.col(cx.sch.font_color, transparency.max(0.0));
    if let Some(ic) = &icon {
        let tint_base = if matches!(ic, IconRef::Lucide(_)) { cx.sch.accent } else { cx.sch.white };
        let tint = cx.col(tint_base, transparency.max(0.0));
        let ir = if compact { content } else { Rect::from_min_size(content.min, Vec2::splat(content.height())) };
        cx.icon(ic, ir, tint);
    }
    if !compact {
        let label_x = content.min.x + cx.px(SIDEBAR_INDENT);
        let right_inset = if has_subs { cx.px(18.0) } else { 0.0 };
        let lr = Rect::from_min_max(Pos2::new(label_x, content.min.y), Pos2::new(content.max.x - right_inset, content.max.y));
        let t = cx.truncate(&name, 16.0, lr.width(), true);
        cx.text(lr, &t, 16.0, font_c, Align::Min, false, true);
    }
    if compact {
        if let Some(tt) = tooltip.as_deref().or(Some(&name)) {
            if hovered {
                cx.tooltip(id, tt);
            }
        }
    }
    // Chevron for tabs with sub tabs.
    let mut chevron_rect = Rect::NOTHING;
    if has_subs && !compact {
        let cr = Rect::from_center_size(Pos2::new(content.max.x - cx.px(8.0), r.center().y), Vec2::splat(cx.px(16.0)));
        chevron_rect = cr;
        let rot = match &mut m.nodes[id].kind {
            Kind::Tab(t) => {
                let animate = m.settings.animations.sidebar_sub_tabs;
                let mut a = t.chevron;
                if animate {
                    a.set(if expanded { 1.0 } else { 0.0 }, cx.time, tw::CHEVRON.0, tw::CHEVRON.1);
                } else {
                    a.snap(if expanded { 1.0 } else { 0.0 });
                }
                let v = cx.anim_value(&a);
                t.chevron = a;
                v
            }
            _ => 0.0,
        };
        let tint = cx.col(cx.sch.font_color, 0.5);
        cx.icon_rotated(&IconRef::Lucide("chevron-down".into()), cr, tint, rot * std::f32::consts::PI);
    }
    // Click.
    if cx.clicked(r) {
        if chevron_rect.contains(cx.pointer().unwrap_or(Pos2::ZERO)) {
            if let Kind::Tab(t) = &mut m.nodes[id].kind {
                t.sidebar_expanded = !t.sidebar_expanded;
            }
        } else if is_active && has_subs {
            if let Kind::Tab(t) = &mut m.nodes[id].kind {
                t.sidebar_expanded = !t.sidebar_expanded;
            }
        } else {
            show(m, id);
        }
    }
    let mut total = bh;
    // Nested sidebar list.
    if has_subs {
        let entry_h = cx.px(30.0);
        let visible_entries: Vec<NodeId> = subs.iter().copied().filter(|s| m.nodes[*s].shown()).collect();
        let full = entry_h * visible_entries.len() as f32;
        let expanded_now = match &m.nodes[id].kind { Kind::Tab(t) => t.sidebar_expanded, _ => false };
        let list_h = match &mut m.nodes[id].kind {
            Kind::Tab(t) => {
                let animate = m.settings.animations.sidebar_sub_tabs;
                let mut a = t.sidebar_list;
                if animate {
                    a.set(if expanded_now { full } else { 0.0 }, cx.time, tw::GROUPBOX.0, tw::GROUPBOX.1);
                } else {
                    a.snap(if expanded_now { full } else { 0.0 });
                }
                let v = cx.anim_value(&a);
                t.sidebar_list = a;
                v
            }
            _ => 0.0,
        };
        if list_h > 0.5 {
            let list_rect = Rect::from_min_size(Pos2::new(r.min.x, r.max.y), Vec2::new(w, list_h));
            cx.with_clip(list_rect, |cx| {
                let mut y = r.max.y;
                for s in visible_entries {
                    pass_sidebar_entry(m, id, s, cx, Pos2::new(r.min.x, y), w, entry_h, compact);
                    y += entry_h;
                }
            });
            total += list_h;
        }
    }
    total
}

fn pass_sidebar_entry(m: &mut Model, tab: NodeId, sub: NodeId, cx: &mut Cx, pos: Pos2, w: f32, h: f32, compact: bool) {
    let r = Rect::from_min_size(pos, Vec2::new(w, h));
    let hovered = cx.hovered(r);
    let (name, icon) = match &m.nodes[sub].kind { Kind::SubTab(s) => (s.name.clone(), s.icon.clone()), _ => return };
    let (active_v, hover_v) = match &mut m.nodes[sub].kind {
        Kind::SubTab(s) => {
            let av = cx.anim_value(&s.entry_active);
            let mut a = s.entry_hover;
            let hv = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
            s.entry_hover = a;
            (av, hv)
        }
        _ => (0.0, 0.0),
    };
    cx.rect(r, 0.0, cx.col(cx.sch.main, 1.0 - 0.5 * active_v));
    let idle = SUBTAB_IDLE;
    let label_t = (idle - 0.2 * hover_v * (1.0 - active_v)) * (1.0 - active_v);
    let label_color = crate::color::lerp(cx.sch.font_color, cx.sch.accent, active_v);
    if compact {
        let ir = Rect::from_center_size(r.center(), Vec2::splat(cx.px(15.0)));
        match &icon {
            Some(ic) => {
                let base = if matches!(ic, IconRef::Lucide(_)) { cx.sch.font_color } else { cx.sch.white };
                cx.icon(ic, ir, cx.col(base, label_t));
            }
            None => {
                let t = cx.truncate(&name, 14.0, r.width() - cx.px(8.0), true);
                cx.text(r, &t, 14.0, cx.col(label_color, label_t), Align::Center, false, true);
            }
        }
        if hovered {
            cx.tooltip(sub, &name);
        }
    } else {
        // Marker.
        let mr = Rect::from_center_size(Pos2::new(r.min.x + cx.px(14.0) + cx.px(1.0), r.center().y), Vec2::new(cx.px(2.0), cx.px(16.0)));
        cx.rect(mr, cx.pill(mr), cx.col(cx.sch.accent, 1.0 - active_v));
        if let Some(ic) = &icon {
            let ir = Rect::from_center_size(Pos2::new(r.min.x + cx.px(SIDEBAR_INDENT + 7.0), r.center().y), Vec2::splat(cx.px(14.0)));
            let base = if matches!(ic, IconRef::Lucide(_)) { cx.sch.font_color } else { cx.sch.white };
            cx.icon(ic, ir, cx.col(base, label_t));
        }
        let lr = Rect::from_min_max(Pos2::new(r.min.x + cx.px(SIDEBAR_INDENT + SIDEBAR_ICON_COL), r.min.y), Pos2::new(r.max.x - cx.px(10.0), r.max.y));
        let t = cx.truncate(&name, 14.0, lr.width(), true);
        cx.text(lr, &t, 14.0, cx.col(label_color, label_t), Align::Min, false, true);
    }
    if cx.clicked(r) {
        let win_active = m.window.and_then(|w| match &m.nodes[w].kind { Kind::Window(wd) => wd.active_tab, _ => None });
        if win_active != Some(tab) {
            show(m, tab);
        }
        show(m, sub);
    }
}

// ----- content ----------------------------------------------------------------------------

pub(crate) fn pass_content(m: &mut Model, win: NodeId, cx: &mut Cx, inner: Rect) {
    let active = match &m.nodes[win].kind { Kind::Window(w) => w.active_tab, _ => None };
    let Some(t) = active else { return };
    let (swipe_from, swipe_off) = match &m.nodes[win].kind { Kind::Window(w) => (w.tab_swipe_from, w.tab_swipe_offset), _ => (SwipeFrom::Bottom, 26.0) };
    match &m.nodes[t].kind {
        Kind::Tab(_) => pass_tab(m, t, cx, inner, swipe_from, swipe_off),
        Kind::KeyTab(_) => pass_key_tab(m, t, cx, inner, swipe_from, swipe_off),
        _ => {}
    }
}

fn canvas_transform(cx: &mut Cx, anim: &Anim, swipe_from: SwipeFrom, swipe_off: f32) -> (Vec2, f32) {
    let v = cx.anim_value(anim);
    let off = cx.px(swipe_off) * (1.0 - v);
    let d = match swipe_from {
        SwipeFrom::Left => Vec2::new(-off, 0.0),
        SwipeFrom::Right => Vec2::new(off, 0.0),
        SwipeFrom::Top => Vec2::new(0.0, -off),
        SwipeFrom::Bottom => Vec2::new(0.0, off),
    };
    (d, v)
}

fn pass_tab(m: &mut Model, tab: NodeId, cx: &mut Cx, inner: Rect, swipe_from: SwipeFrom, swipe_off: f32) {
    let canvas = match &m.nodes[tab].kind { Kind::Tab(t) => t.canvas, _ => return };
    let (delta, alpha) = canvas_transform(cx, &canvas, swipe_from, swipe_off);
    let prev_fade = cx.fade;
    cx.fade = prev_fade * alpha;
    let rect = inner.translate(delta);
    m.nodes[tab].rect = rect;

    let mut offset = 0.0;
    // Warning box.
    offset = pass_warning_box(m, tab, cx, rect, offset);
    // Banners.
    offset = pass_banners(m, tab, cx, rect, offset);
    // Sub tabs.
    let (subs, active_sub, layout) = match &m.nodes[tab].kind { Kind::Tab(t) => (t.sub_tabs.clone(), t.active_sub_tab, t.layout), _ => return };
    let has_subs = subs.iter().any(|s| m.nodes[*s].shown());
    if has_subs {
        offset = pass_sub_tab_bar(m, tab, cx, rect, offset, &subs);
        if let Some(st) = active_sub {
            let canvas = match &m.nodes[st].kind { Kind::SubTab(s) => s.canvas, _ => return };
            let (d2, a2) = canvas_transform(cx, &canvas, SwipeFrom::Bottom, swipe_off);
            let f = cx.fade;
            cx.fade = f * a2;
            let cols = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + offset), rect.max).translate(d2);
            let boxes = match &m.nodes[st].kind { Kind::SubTab(s) => s.boxes.clone(), _ => Vec::new() };
            let mut scroll = match &m.nodes[st].kind { Kind::SubTab(s) => s.scroll, _ => [Scroll::default(); 2] };
            pass_columns(m, cx, cols, layout, &boxes, &mut scroll);
            if let Kind::SubTab(s) = &mut m.nodes[st].kind {
                s.scroll = scroll;
            }
            cx.fade = f;
        }
    } else {
        let cols = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + offset), rect.max);
        let boxes = match &m.nodes[tab].kind { Kind::Tab(t) => t.boxes.clone(), _ => Vec::new() };
        let mut scroll = match &m.nodes[tab].kind { Kind::Tab(t) => t.scroll, _ => [Scroll::default(); 2] };
        pass_columns(m, cx, cols, layout, &boxes, &mut scroll);
        if let Kind::Tab(t) = &mut m.nodes[tab].kind {
            t.scroll = scroll;
        }
    }
    cx.fade = prev_fade;
}

fn pass_warning_box(m: &mut Model, tab: NodeId, cx: &mut Cx, rect: Rect, offset: f32) -> f32 {
    let wb = match &m.nodes[tab].kind { Kind::Tab(t) => t.warning.clone(), _ => return offset };
    if !wb.visible {
        return offset;
    }
    let x = rect.min.x + cx.px(2.0);
    let w = rect.width() - cx.px(5.0);
    let text_w = w - cx.px(12.0) - cx.px(4.0);
    let text_h = if wb.text.is_empty() { 0.0 } else { cx.text_size(&wb.text, 14.0, Some(text_w), true).y };
    let mut box_h = cx.px(24.0) + text_h;
    let max_h = (rect.height() / 3.25).floor();
    let mut inner_scroll = false;
    if wb.lock_size && box_h >= max_h {
        box_h = max_h;
        inner_scroll = true;
    }
    let r = Rect::from_min_size(Pos2::new(x, rect.min.y + cx.px(7.0)), Vec2::new(w, box_h + cx.px(4.0)));
    let (bg, outline, shadow, title_c, stroke_c) = if wb.is_normal {
        (cx.sch.background, cx.sch.outline, cx.sch.dark, cx.sch.font_color, cx.sch.outline)
    } else {
        (
            epaint::Color32::from_rgb(127, 0, 0),
            epaint::Color32::from_rgb(255, 50, 50),
            epaint::Color32::from_rgb(85, 0, 0),
            epaint::Color32::from_rgb(255, 50, 50),
            epaint::Color32::from_rgb(169, 0, 0),
        )
    };
    let corners = Corners::same(cx.r());
    cx.rect(r, corners, cx.col(bg, 0.0));
    let mut v = Vec::new();
    crate::draw::outline(r, corners, cx.col(outline, 0.0), cx.col(shadow, 0.0), cx.m.s, &mut v);
    cx.push_all(v);
    let pad = Rect::from_min_max(Pos2::new(r.min.x + cx.px(6.0), r.min.y + cx.px(4.0)), Pos2::new(r.max.x - cx.px(6.0), r.max.y - cx.px(4.0)));
    let title_r = Rect::from_min_size(pad.min, Vec2::new(pad.width() - cx.px(4.0), cx.px(14.0)));
    let _ = stroke_c;
    cx.text(title_r, &wb.title, 14.0, cx.col(title_c, 0.0), Align::Min, false, true);
    let text_r = Rect::from_min_max(Pos2::new(pad.min.x, pad.min.y + cx.px(16.0)), Pos2::new(pad.max.x - cx.px(4.0), pad.max.y));
    let mut scroll = wb.scroll;
    scroll.view = text_r.height();
    scroll.content = text_h;
    if inner_scroll {
        let wheel = cx.wheel_over(r);
        if wheel != 0.0 {
            scroll.wheel(wheel);
        }
    } else {
        scroll.offset = 0.0;
    }
    scroll.clamp();
    let tr = Rect::from_min_size(Pos2::new(text_r.min.x, text_r.min.y - scroll.offset), Vec2::new(text_r.width(), text_h.max(1.0)));
    cx.with_clip(text_r, |cx| {
        cx.text_top(tr, &wb.text, 14.0, cx.col(cx.sch.font_color, 0.0), Align::Min, true);
    });
    if inner_scroll && scroll.scrollable() {
        let track = Rect::from_min_max(Pos2::new(r.max.x - cx.px(3.0), text_r.min.y), Pos2::new(r.max.x, text_r.max.y));
        let thumb = scroll.thumb(track);
        cx.rect(thumb, cx.pill(thumb), cx.col(cx.sch.outline, 0.0));
    }
    if let Kind::Tab(t) = &mut m.nodes[tab].kind {
        t.warning.scroll = scroll;
    }
    offset + r.height() + cx.px(8.0) - cx.px(0.0)
}

fn pass_banners(m: &mut Model, tab: NodeId, cx: &mut Cx, rect: Rect, offset: f32) -> f32 {
    let banners = match &m.nodes[tab].kind { Kind::Tab(t) => t.banners.clone(), _ => return offset };
    let shown: Vec<NodeId> = banners.into_iter().filter(|b| m.nodes[*b].shown()).collect();
    if shown.is_empty() {
        return offset;
    }
    let x = rect.min.x + cx.px(2.0);
    let w = rect.width() - cx.px(5.0);
    let mut y = rect.min.y + offset + cx.px(7.0);
    let start = y;
    for b in shown {
        let h = crate::elements::media::pass_profile(m, b, cx, x, y, w);
        y += h + cx.px(6.0);
    }
    let height = y - start - cx.px(6.0);
    offset + cx.px(7.0) + height + cx.px(1.0)
}

fn pass_sub_tab_bar(m: &mut Model, tab: NodeId, cx: &mut Cx, rect: Rect, offset: f32, subs: &[NodeId]) -> f32 {
    let bar = Rect::from_min_size(Pos2::new(rect.min.x + cx.px(2.0), rect.min.y + offset), Vec2::new(rect.width() - cx.px(4.0), cx.px(SUBTAB_BAR_H)));
    let (align, active) = match &m.nodes[tab].kind { Kind::Tab(t) => (t.sub_tab_align, t.active_sub_tab), _ => return offset };
    // Measure chips.
    let mut chips: Vec<(NodeId, f32, String, Option<IconRef>)> = Vec::new();
    let mut total = 0.0;
    for s in subs {
        if !m.nodes[*s].shown() {
            continue;
        }
        let (name, icon) = match &m.nodes[*s].kind { Kind::SubTab(st) => (st.name.clone(), st.icon.clone()), _ => continue };
        let tw_ = cx.text_size(&name, 15.0, None, true).x.ceil();
        let iw = if icon.is_some() { cx.px(SUBTAB_ICON + 6.0) } else { 0.0 };
        let w = tw_ + iw + cx.px(24.0);
        chips.push((*s, w, name, icon));
        total += w;
    }
    let gap = cx.px(6.0);
    total += gap * chips.len().saturating_sub(1) as f32;
    let mut x = match align {
        Align_::Left => bar.min.x,
        Align_::Center => bar.center().x - total / 2.0,
        Align_::Right => bar.max.x - total,
    };
    let chip_h = cx.px(SUBTAB_BAR_H - 8.0);
    let mut active_rect = None;
    for (s, w, name, icon) in chips {
        let r = Rect::from_center_size(Pos2::new(x + w / 2.0, bar.center().y), Vec2::new(w, chip_h));
        if active == Some(s) {
            active_rect = Some(r);
        }
        pass_sub_tab_chip(m, s, cx, r, &name, icon.as_ref(), active == Some(s));
        x += w + gap;
    }
    // Underline.
    if let Some(ar) = active_rect {
        let line_w = (ar.width() * SUBTAB_UNDERLINE_W).floor();
        let target_x = (ar.min.x + (ar.width() - line_w) / 2.0).floor();
        let bottom = ar.max.y - cx.px(SUBTAB_UNDERLINE_GAP);
        let animate = m.settings.animations.sub_tab_underline;
        let (ux, uw) = match &mut m.nodes[tab].kind {
            Kind::Tab(t) => {
                if !t.underline_visible {
                    t.underline_x.snap(target_x);
                    t.underline_w.snap(line_w);
                    t.underline_visible = true;
                } else if animate {
                    t.underline_x.set(target_x, cx.time, tw::SUBTAB_SLIDE.0, tw::SUBTAB_SLIDE.1);
                    t.underline_w.set(line_w, cx.time, tw::SUBTAB_SLIDE.0, tw::SUBTAB_SLIDE.1);
                } else {
                    t.underline_x.snap(target_x);
                    t.underline_w.snap(line_w);
                }
                (cx.anim_value(&t.underline_x), cx.anim_value(&t.underline_w))
            }
            _ => (target_x, line_w),
        };
        let ur = Rect::from_min_size(Pos2::new(ux, bottom), Vec2::new(uw, cx.px(1.0).max(1.0)));
        let mid = cx.col(cx.sch.accent, 0.1);
        let edge = cx.col(cx.sch.font_color, 1.0);
        let half = Rect::from_min_max(ur.min, Pos2::new(ur.center().x, ur.max.y));
        cx.push(crate::draw::gradient_rect(half, edge, mid, true));
        let half2 = Rect::from_min_max(Pos2::new(ur.center().x, ur.min.y), ur.max);
        cx.push(crate::draw::gradient_rect(half2, mid, edge, true));
    }
    offset + cx.px(SUBTAB_BAR_H + 6.0)
}

fn pass_sub_tab_chip(m: &mut Model, sub: NodeId, cx: &mut Cx, r: Rect, name: &str, icon: Option<&IconRef>, is_active: bool) {
    let hovered = cx.hovered(r);
    let (active_v, hover_v, scale) = match &mut m.nodes[sub].kind {
        Kind::SubTab(s) => {
            let av = cx.anim_value(&s.active);
            let mut h = s.chip;
            let hv = cx.tween(&mut h, if hovered && !is_active { 1.0 } else { 0.0 }, tw::HOVER);
            s.chip = h;
            let mut sc = s.scale;
            let sv = cx.tween(&mut sc, if hovered { SUBTAB_HOVER_SCALE } else { 1.0 }, tw::SUBTAB_HOVER);
            s.scale = sc;
            (av, hv, sv)
        }
        _ => (0.0, 0.0, 1.0),
    };
    let vr = Rect::from_center_size(r.center(), r.size() * scale);
    // Shadows.
    for (i, t) in SUBTAB_SHADOW.iter().enumerate() {
        let tr = 1.0 - active_v * (1.0 - t) - hover_v * (1.0 - (t + 0.2).min(1.0)) * (1.0 - active_v);
        if tr < 0.999 {
            let sr = vr.translate(Vec2::new(0.0, cx.px((i + 1) as f32)));
            cx.rect(sr, cx.r(), cx.col(cx.sch.dark, tr.clamp(0.0, 1.0)));
        }
    }
    let chip_bg = cx.sch.better(cx.sch.main, 10.0);
    let bg_t = 1.0 - active_v - 0.55 * hover_v * (1.0 - active_v);
    cx.rect(vr, cx.r(), cx.col(chip_bg, bg_t.clamp(0.0, 1.0)));
    let stroke_t = 1.0 - 0.75 * active_v - 0.3 * hover_v * (1.0 - active_v);
    cx.stroke(vr, cx.r(), 1.0, cx.col(cx.sch.outline, stroke_t.clamp(0.0, 1.0)), StrokeKind::Inside);
    let label_t = (SUBTAB_IDLE - 0.3 * hover_v) * (1.0 - active_v);
    let label_c = crate::color::lerp(cx.sch.font_color, cx.sch.accent, active_v);
    let mut x = vr.min.x + cx.px(12.0) * scale;
    if let Some(ic) = icon {
        let isz = cx.px(SUBTAB_ICON) * scale;
        let ir = Rect::from_center_size(Pos2::new(x + isz / 2.0, vr.center().y), Vec2::splat(isz));
        let base = if matches!(ic, IconRef::Lucide(_)) { cx.sch.accent } else { cx.sch.white };
        cx.icon(ic, ir, cx.col(base, label_t));
        x += isz + cx.px(6.0) * scale;
    }
    let lr = Rect::from_min_max(Pos2::new(x, vr.min.y), Pos2::new(vr.max.x - cx.px(12.0) * scale, vr.max.y));
    cx.text(lr, name, 15.0 * scale, cx.col(label_c, label_t), Align::Center, false, true);
    if cx.clicked(r) {
        show(m, sub);
    }
}

/// Lay out two (or one) columns of boxes with independent scrolling.
pub(crate) fn pass_columns(m: &mut Model, cx: &mut Cx, rect: Rect, layout: Layout, boxes: &[NodeId], scroll: &mut [Scroll; 2]) {
    let sides: &[Side] = if layout == Layout::Single { &[Side::Left] } else { &[Side::Left, Side::Right] };
    for (i, side) in sides.iter().enumerate() {
        let col = if layout == Layout::Single {
            rect
        } else {
            let w = rect.width() * 0.5 - cx.px(3.0);
            match side {
                Side::Left => Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
                Side::Right => Rect::from_min_size(Pos2::new(rect.max.x - w, rect.min.y), Vec2::new(w, rect.height())),
            }
        };
        let sc = &mut scroll[i];
        sc.view = col.height();
        let wheel = cx.wheel_over(col);
        if wheel != 0.0 && m.capture.is_none() {
            sc.wheel(wheel);
        }
        sc.clamp();
        let inner_x = col.min.x + cx.px(2.0);
        let inner_w = col.width() - cx.px(4.0);
        let mut y = col.min.y + cx.px(4.0) - sc.offset;
        let start = y;
        cx.with_clip(col, |cx| {
            for b in boxes {
                if !m.alive(*b) {
                    continue;
                }
                let bside = match &m.nodes[*b].kind {
                    Kind::Groupbox(g) => g.side,
                    Kind::Tabbox(t) => t.side,
                    Kind::DepGroupbox(d) => d.side,
                    _ => continue,
                };
                let bside = if layout == Layout::Single { Side::Left } else { bside };
                if bside != *side {
                    continue;
                }
                if !m.nodes[*b].shown() {
                    continue;
                }
                let h = super::boxes::pass_box(m, *b, cx, inner_x, y + cx.px(4.0), inner_w);
                if h > 0.0 {
                    y += h + cx.px(8.0) + cx.px(2.0);
                }
            }
        });
        sc.content = y - start + cx.px(4.0);
    }
}

fn pass_key_tab(m: &mut Model, tab: NodeId, cx: &mut Cx, inner: Rect, swipe_from: SwipeFrom, swipe_off: f32) {
    let canvas = match &m.nodes[tab].kind { Kind::KeyTab(t) => t.canvas, _ => return };
    let (delta, alpha) = canvas_transform(cx, &canvas, swipe_from, swipe_off);
    let prev_fade = cx.fade;
    cx.fade = prev_fade * alpha;
    let rect = inner.translate(delta);
    m.nodes[tab].rect = rect;
    let mut scroll = match &m.nodes[tab].kind { Kind::KeyTab(t) => t.scroll, _ => Scroll::default() };
    scroll.view = rect.height();
    let wheel = cx.wheel_over(rect);
    if wheel != 0.0 {
        scroll.wheel(wheel);
    }
    scroll.clamp();
    // Measure children first to center vertically.
    let children = m.nodes[tab].children.clone();
    let gap = cx.px(8.0);
    let x = rect.min.x + cx.px(1.0);
    let w = rect.width() - cx.px(2.0);
    let content_h = crate::elements::measure_children(m, &children, cx, w, gap);
    let start_y = if content_h < rect.height() { rect.center().y - content_h / 2.0 } else { rect.min.y - scroll.offset };
    cx.with_clip(rect, |cx| {
        let _ = crate::elements::pass_children(m, &children, cx, x, start_y, w, gap, true);
    });
    scroll.content = content_h;
    if let Kind::KeyTab(t) = &mut m.nodes[tab].kind {
        t.scroll = scroll;
    }
    cx.fade = prev_fade;
}

/// Key box row: text field + "Execute" button, 75% wide centered.
pub(crate) fn pass_key_box(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let h = cx.px(21.0);
    let row_w = w * 0.75;
    let rx = x + (w - row_w) / 2.0;
    let r = Rect::from_min_size(Pos2::new(rx, y), Vec2::new(row_w, h));
    m.nodes[id].rect = r;
    let field = cx.field(id, 0).cloned();
    let focused = field.as_ref().map(|f| f.focused).unwrap_or(false);
    let text = {
        let Kind::KeyBox(k) = &mut m.nodes[id].kind else { return 0.0 };
        if let Some(f) = &field {
            k.text = f.text.clone();
        }
        k.text.clone()
    };
    let box_r = Rect::from_min_max(r.min, Pos2::new(r.max.x - cx.px(71.0), r.max.y));
    let btn_r = Rect::from_min_max(Pos2::new(r.max.x - cx.px(63.0), r.min.y), r.max);
    let stroke_c = match &mut m.nodes[id].kind {
        Kind::KeyBox(k) => {
            let mut a = k.focus;
            let v = cx.tween(&mut a, if focused { 1.0 } else { 0.0 }, tw::HOVER);
            k.focus = a;
            crate::color::lerp(cx.sch.outline, cx.sch.accent, v)
        }
        _ => cx.sch.outline,
    };
    cx.rect(box_r, cx.r2(), cx.col(cx.sch.main, 0.0));
    cx.stroke(box_r, cx.r2(), 1.0, cx.col(stroke_c, 0.0), StrokeKind::Inside);
    let fr = Rect::from_min_max(Pos2::new(box_r.min.x + cx.px(8.0), box_r.min.y + cx.px(2.0)), Pos2::new(box_r.max.x - cx.px(8.0), box_r.max.y - cx.px(2.0)));
    if m.open && cx.fade > 0.99 && !cx.content_covered && !crate::window::covered(m, cx.layer, box_r) {
        cx.request_field(TextFieldRequest {
            id,
            sub: 0,
            rect: fr,
            text: text.clone(),
            placeholder: "Key".to_owned(),
            font: cx.font(14.0),
            color: cx.col(cx.sch.font_color, 0.0),
            placeholder_color: cx.col(cx.sch.placeholder(), 0.0),
            focus: FocusRequest::None,
            numeric: false,
            max_len: None,
            clear_on_focus: false,
            password: false,
            layer: Layer::Window,
        });
    } else {
        cx.text(fr, if text.is_empty() { "Key" } else { &text }, 14.0, cx.col(if text.is_empty() { cx.sch.placeholder() } else { cx.sch.font_color }, 0.0), Align::Min, false, false);
    }
    let hovered = cx.hovered(btn_r);
    let hv = {
        let n = &mut m.nodes[id];
        let mut a = n.hover;
        let v = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
        n.hover = a;
        v
    };
    cx.rect(btn_r, cx.r2(), cx.col(cx.sch.main, 0.0));
    cx.stroke(btn_r, cx.r2(), 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Inside);
    cx.text(btn_r, "Execute", 14.0, cx.col(cx.sch.font_color, 0.4 * (1.0 - hv)), Align::Center, false, false);
    if cx.clicked(btn_r) {
        m.pending.push(crate::ui::Pending::Text(id, text));
    }
    let _ = MouseButton::Left;
    h
}
