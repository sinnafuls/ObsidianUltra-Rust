//! Groupboxes and tabboxes (in-column and popped-out rendering).

use epaint::emath::Align;
use epaint::{Pos2, Rect, Vec2};

use crate::draw::Corners;
use crate::frame::Cx;
use crate::handles::info::{GroupboxInfo, TabboxInfo};
use crate::node::*;
use crate::tween::{info as tw, Anim, Ease};
use crate::types::*;
use crate::ui::Model;

use super::popout;

pub(crate) fn create_groupbox(m: &mut Model, owner: NodeId, info: GroupboxInfo) -> NodeId {
    let data = GroupboxData {
        side: info.side,
        name: info.name,
        icon: info.icon,
        description: info.description,
        collapsed: false,
        disable_collapsing: info.disable_collapsing,
        chevron: Anim::new(1.0),
        height: Anim::new(0.0),
        pop: PopOut::new(info.pop_out),
        content_height: 0.0,
    };
    let id = m.insert(Kind::Groupbox(data), Some(owner));
    match &mut m.nodes[owner].kind {
        Kind::Tab(t) => t.boxes.push(id),
        Kind::SubTab(t) => t.boxes.push(id),
        _ => m.nodes[owner].children.push(id),
    }
    if !info.visible {
        m.nodes[id].visible = false;
    }
    if info.collapsed && !info.disable_collapsing {
        if let Kind::Groupbox(g) = &mut m.nodes[id].kind {
            g.collapsed = true;
            g.chevron.snap(0.0);
        }
    }
    id
}

pub(crate) fn create_tabbox(m: &mut Model, owner: NodeId, info: TabboxInfo) -> NodeId {
    let in_groupbox = matches!(m.nodes[owner].kind, Kind::Groupbox(_));
    let data = TabboxData {
        side: info.side,
        name: info.name,
        tabs: Vec::new(),
        active: None,
        in_groupbox,
        underline_x: Anim::new(0.0),
        underline_w: Anim::new(0.0),
        underline_visible: false,
        pop: PopOut::new(info.pop_out),
        height: 0.0,
    };
    let id = m.insert(Kind::Tabbox(data), Some(owner));
    match &mut m.nodes[owner].kind {
        Kind::Tab(t) => t.boxes.push(id),
        Kind::SubTab(t) => t.boxes.push(id),
        _ => m.nodes[owner].children.push(id),
    }
    id
}

pub(crate) fn create_tabbox_tab(m: &mut Model, tabbox: NodeId, name: Option<&str>, icon: Option<IconRef>) -> NodeId {
    let name = name.map(|s| s.trim()).filter(|s| !s.is_empty()).map(|s| s.to_owned());
    let data = TabboxTabData { name, icon, slide: Anim::new(0.0), active: Anim::new(0.0) };
    let id = m.insert(Kind::TabboxTab(data), Some(tabbox));
    let first = if let Kind::Tabbox(t) = &mut m.nodes[tabbox].kind {
        t.tabs.push(id);
        t.active.is_none()
    } else {
        false
    };
    if first {
        show_tabbox_tab(m, id);
    }
    id
}

pub(crate) fn show_tabbox_tab(m: &mut Model, id: NodeId) {
    let Some(tabbox) = m.nodes[id].parent else { return };
    let time = m.time;
    let animate = m.settings.animations.tab_switch;
    let transition = m.window.map(|w| match &m.nodes[w].kind { Kind::Window(wd) => wd.tab_transition_time, _ => 0.22 }).unwrap_or(0.22);
    let (prev, popped) = match &m.nodes[tabbox].kind { Kind::Tabbox(t) => (t.active, t.pop.popped), _ => return };
    if prev == Some(id) {
        return;
    }
    if let Some(p) = prev {
        if let Kind::TabboxTab(t) = &mut m.nodes[p].kind {
            t.active.set(0.0, time, tw::HOVER.0, tw::HOVER.1);
        }
    }
    if let Kind::Tabbox(t) = &mut m.nodes[tabbox].kind {
        t.active = Some(id);
    }
    if let Kind::TabboxTab(t) = &mut m.nodes[id].kind {
        t.active.snap(1.0);
        if prev.is_some() && animate && !popped {
            t.slide.play(1.0, 0.0, time, transition, Ease::QuadOut);
        } else {
            t.slide.snap(0.0);
        }
    }
}

pub(crate) fn hide_tabbox_tab(m: &mut Model, id: NodeId) {
    let Some(tabbox) = m.nodes[id].parent else { return };
    if let Kind::Tabbox(t) = &mut m.nodes[tabbox].kind {
        if t.active == Some(id) {
            t.active = None;
        }
    }
    if let Kind::TabboxTab(t) = &mut m.nodes[id].kind {
        t.active.snap(0.0);
    }
}

/// Lay out one box (groupbox / tabbox / dependency groupbox) in a column. Returns its height.
pub(crate) fn pass_box(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    match &m.nodes[id].kind {
        Kind::Groupbox(g) => {
            if g.pop.popped {
                pass_placeholder(m, id, cx, x, y, w)
            } else {
                draw_groupbox(m, id, cx, x, y, w, false)
            }
        }
        Kind::Tabbox(t) => {
            if t.pop.popped {
                pass_placeholder(m, id, cx, x, y, w)
            } else {
                draw_tabbox(m, id, cx, x, y, w, false)
            }
        }
        Kind::DepGroupbox(_) => crate::elements::dependency::pass_dep_groupbox(m, id, cx, x, y, w),
        _ => 0.0,
    }
}

/// Dimmed placeholder left behind by a popped-out box.
fn pass_placeholder(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let header_h = match &m.nodes[id].kind {
        Kind::Groupbox(_) => groupbox_top_height(m, id, cx, w),
        Kind::Tabbox(_) => cx.px(34.0),
        _ => return 0.0,
    };
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, header_h));
    let corners = Corners::same(cx.r());
    cx.rect(r, corners, cx.col(cx.sch.background, 0.12));
    let prev = cx.fade;
    cx.fade = prev * 0.55;
    match &m.nodes[id].kind {
        Kind::Groupbox(_) => {
            draw_groupbox_top(m, id, cx, r, false);
        }
        Kind::Tabbox(_) => {
            draw_tabbox_header(m, id, cx, r, false);
        }
        _ => {}
    }
    cx.fade = prev;
    cx.outline(r, corners, 0.0);
    // Dock button.
    let br = Rect::from_center_size(Pos2::new(r.max.x - cx.px(8.0 + 11.0), r.center().y), Vec2::splat(cx.px(22.0)));
    let hovered = cx.hovered(br);
    let tint = cx.col(cx.sch.white, if hovered { 0.0 } else { 0.25 });
    cx.icon(&IconRef::Lucide("square-arrow-down-left".into()), br.shrink(cx.px(2.0)), tint);
    if cx.clicked(br) {
        popout::set_popped_out(m, id, false, None);
    }
    match &mut m.nodes[id].kind {
        Kind::Groupbox(g) => g.pop.placeholder = r,
        Kind::Tabbox(t) => t.pop.placeholder = r,
        _ => {}
    }
    r.height()
}

fn groupbox_top_height(m: &mut Model, id: NodeId, cx: &mut Cx, w: f32) -> f32 {
    let Kind::Groupbox(g) = &m.nodes[id].kind else { return 0.0 };
    let (name, desc, has_icon, right_inset) = (g.name.clone(), g.description.clone(), g.icon.is_some(), if g.disable_collapsing { 0.0 } else { 22.0 });
    let text_w = w - cx.px(12.0) - cx.px(right_inset) - if has_icon { cx.px(24.0) } else { 0.0 } - cx.px(12.0);
    let name_h = cx.text_size(&name, 15.0, Some(text_w.max(1.0)), true).y;
    let desc_h = desc.map(|d| cx.text_size(&d, 14.0, Some(text_w.max(1.0)), true).y).unwrap_or(0.0);
    let min_h = if has_icon { cx.px(22.0 + 12.0) } else { 0.0 };
    (name_h + desc_h + cx.px(19.0)).max(min_h)
}

/// Draw a groupbox header (icon, name, description). `interactive` enables the chevron.
fn draw_groupbox_top(m: &mut Model, id: NodeId, cx: &mut Cx, top: Rect, interactive: bool) {
    let Kind::Groupbox(g) = &m.nodes[id].kind else { return };
    let (name, desc, icon, disable_collapsing, collapsed) = (g.name.clone(), g.description.clone(), g.icon.clone(), g.disable_collapsing, g.collapsed);
    let pad = cx.px(6.0);
    let right_inset = if disable_collapsing { 0.0 } else { cx.px(22.0) };
    let mut tx = top.min.x + pad;
    if let Some(ic) = &icon {
        let ir = Rect::from_center_size(Pos2::new(tx + cx.px(11.0), top.center().y), Vec2::splat(cx.px(22.0)));
        let base = if matches!(ic, IconRef::Lucide(_)) { cx.sch.accent } else { cx.sch.white };
        cx.icon(ic, ir, cx.col(base, 0.0));
        tx += cx.px(24.0);
    }
    let texts = Rect::from_min_max(Pos2::new(tx + cx.px(6.0), top.min.y + pad + cx.px(3.0)), Pos2::new(top.max.x - pad - right_inset - cx.px(6.0), top.max.y - pad - cx.px(3.0)));
    let name_h = cx.text_size(&name, 15.0, Some(texts.width().max(1.0)), true).y;
    let nr = Rect::from_min_size(texts.min, Vec2::new(texts.width(), name_h));
    cx.text_top(nr, &name, 15.0, cx.col(cx.sch.font_color, 0.0), Align::Min, true);
    if let Some(d) = &desc {
        let dr = Rect::from_min_max(Pos2::new(texts.min.x, nr.max.y + cx.px(1.0)), texts.max);
        cx.text_top(dr, d, 14.0, cx.col(cx.sch.font_color, 0.5), Align::Min, true);
    }
    if !disable_collapsing {
        let cr = Rect::from_center_size(Pos2::new(top.max.x - pad - cx.px(11.0), top.center().y), Vec2::splat(cx.px(22.0)));
        let rot = {
            let Kind::Groupbox(g) = &mut m.nodes[id].kind else { return };
            let animate = m.settings.animations.groupbox;
            let mut a = g.chevron;
            if animate {
                a.set(if collapsed { 0.0 } else { 1.0 }, cx.time, tw::CHEVRON.0, tw::CHEVRON.1);
            } else {
                a.snap(if collapsed { 0.0 } else { 1.0 });
            }
            let v = cx.anim_value(&a);
            g.chevron = a;
            v
        };
        cx.icon_rotated(&IconRef::Lucide("chevron-up".into()), cr, cx.col(cx.sch.white, 0.0), rot * std::f32::consts::PI);
        if interactive && cx.clicked(cr) {
            if let Kind::Groupbox(g) = &mut m.nodes[id].kind {
                g.collapsed = !g.collapsed;
            }
        }
    }
}

/// Draw a full groupbox at `(x, y)` with width `w`. Used in-column and in pop-out floats.
pub(crate) fn draw_groupbox(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32, in_float: bool) -> f32 {
    let top_h = groupbox_top_height(m, id, cx, w);
    let collapsed = match &m.nodes[id].kind { Kind::Groupbox(g) => g.collapsed, _ => return 0.0 };
    let children = m.nodes[id].children.clone();
    let gap = cx.px(8.0);
    let inner_w = w - cx.px(14.0);
    let content_h = if collapsed { 0.0 } else { crate::elements::measure_children(m, &children, cx, inner_w, gap) + cx.px(14.0) };
    let target_h = if collapsed { top_h } else { top_h + cx.px(1.0) + content_h };
    let height = {
        let Kind::Groupbox(g) = &mut m.nodes[id].kind else { return 0.0 };
        let animate = m.settings.animations.groupbox;
        let mut a = g.height;
        if a.target() == 0.0 || !animate {
            a.snap(target_h);
        } else {
            a.set(target_h, cx.time, tw::GROUPBOX.0, tw::GROUPBOX.1);
        }
        let v = cx.anim_value(&a);
        g.height = a;
        g.content_height = content_h;
        v
    };
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, height));
    m.nodes[id].rect = r;
    let corners = Corners::same(cx.r());
    cx.rect(r, corners, cx.col(cx.sch.background, 0.0));
    let top = Rect::from_min_size(r.min, Vec2::new(w, top_h));
    cx.with_clip(r, |cx| {
        draw_groupbox_top(m, id, cx, top, true);
        if !collapsed {
            cx.hline(r.min.x, r.max.x, top.max.y, cx.col(cx.sch.outline, 0.0));
            let cy = top.max.y + cx.px(1.0) + cx.px(7.0);
            let _ = crate::elements::pass_children(m, &children, cx, x + cx.px(7.0), cy, inner_w, gap, false);
        }
    });
    cx.outline(r, corners, 0.0);
    if let Kind::Groupbox(g) = &mut m.nodes[id].kind {
        g.pop.header = top;
    }
    if !in_float {
        popout::handle_header(m, id, cx, top, r);
    } else {
        popout::handle_header(m, id, cx, top, r);
    }
    height
}

fn draw_tabbox_header(m: &mut Model, id: NodeId, cx: &mut Cx, header: Rect, interactive: bool) {
    let Kind::Tabbox(t) = &m.nodes[id].kind else { return };
    let tabs: Vec<NodeId> = t.tabs.iter().copied().filter(|t| m.alive(*t)).collect();
    let active = t.active;
    if tabs.is_empty() {
        return;
    }
    let bw = header.width() / tabs.len() as f32;
    let mut active_rect = None;
    for (i, tab) in tabs.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(header.min.x + bw * i as f32, header.min.y), Vec2::new(bw, header.height()));
        let (name, icon) = match &m.nodes[*tab].kind { Kind::TabboxTab(tt) => (tt.name.clone(), tt.icon.clone()), _ => continue };
        let is_active = active == Some(*tab);
        if is_active {
            active_rect = Some(r);
        }
        let av = match &mut m.nodes[*tab].kind { Kind::TabboxTab(tt) => cx.anim_value(&tt.active), _ => 0.0 };
        let hovered = interactive && cx.hovered(r);
        let hv = {
            let n = &mut m.nodes[*tab];
            let mut a = n.hover;
            let v = cx.tween(&mut a, if hovered && !is_active { 1.0 } else { 0.0 }, tw::HOVER);
            n.hover = a;
            v
        };
        let t_ = (0.5 - 0.25 * hv) * (1.0 - av);
        let isz = if name.is_some() { 18.0 } else { 16.0 };
        let iw = if icon.is_some() { cx.px(isz) } else { 0.0 };
        let tw_ = name.as_ref().map(|n| cx.text_size(n, 15.0, None, true).x).unwrap_or(0.0);
        let total = iw + tw_ + if icon.is_some() && name.is_some() { cx.px(8.0) } else { 0.0 };
        let mut x = r.center().x - total / 2.0;
        if let Some(ic) = &icon {
            let ir = Rect::from_center_size(Pos2::new(x + iw / 2.0, r.center().y), Vec2::splat(iw));
            let base = if matches!(ic, IconRef::Lucide(_)) { cx.sch.accent } else { cx.sch.white };
            cx.icon(ic, ir, cx.col(base, t_));
            x += iw + cx.px(8.0);
        }
        if let Some(n) = &name {
            let lr = Rect::from_min_size(Pos2::new(x, r.min.y), Vec2::new(tw_, r.height()));
            cx.text(lr, n, 15.0, cx.col(cx.sch.font_color, t_), Align::Min, false, true);
        }
        if interactive && cx.clicked(r) {
            show_tabbox_tab(m, *tab);
        }
    }
    cx.hline(header.min.x, header.max.x, header.max.y, cx.col(cx.sch.outline, 0.0));
    if let Some(ar) = active_rect {
        let target_x = (ar.min.x + cx.px(12.0)).floor();
        let target_w = (ar.width() - cx.px(24.0)).floor().max(0.0);
        let animate = m.settings.animations.sub_tab_underline;
        let (ux, uw) = match &mut m.nodes[id].kind {
            Kind::Tabbox(t) => {
                if !t.underline_visible {
                    t.underline_x.snap(target_x);
                    t.underline_w.snap(target_w);
                    t.underline_visible = true;
                } else if animate {
                    t.underline_x.set(target_x, cx.time, tw::SUBTAB_SLIDE.0, tw::SUBTAB_SLIDE.1);
                    t.underline_w.set(target_w, cx.time, tw::SUBTAB_SLIDE.0, tw::SUBTAB_SLIDE.1);
                } else {
                    t.underline_x.snap(target_x);
                    t.underline_w.snap(target_w);
                }
                (cx.anim_value(&t.underline_x), cx.anim_value(&t.underline_w))
            }
            _ => (target_x, target_w),
        };
        let ur = Rect::from_min_size(Pos2::new(ux, header.max.y + cx.px(1.0) - cx.px(2.0)), Vec2::new(uw, cx.px(2.0)));
        cx.rect(ur, 0.0, cx.col(cx.sch.accent, 0.0));
    }
}

/// Draw a full tabbox at `(x, y)` with width `w`.
pub(crate) fn draw_tabbox(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32, in_float: bool) -> f32 {
    let (active, in_groupbox) = match &m.nodes[id].kind { Kind::Tabbox(t) => (t.active, t.in_groupbox), _ => return 0.0 };
    let header_h = cx.px(34.0);
    let gap = cx.px(8.0);
    let inner_w = w - cx.px(14.0);
    let children = active.map(|a| m.nodes[a].children.clone()).unwrap_or_default();
    let content_h = if active.is_some() { crate::elements::measure_children(m, &children, cx, inner_w, gap) + cx.px(14.0) } else { 0.0 };
    let height = header_h + cx.px(1.0) + content_h;
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, height));
    m.nodes[id].rect = r;
    if let Kind::Tabbox(t) = &mut m.nodes[id].kind {
        t.height = height;
    }
    let corners = Corners::same(cx.r());
    if !in_groupbox || in_float {
        cx.rect(r, corners, cx.col(cx.sch.background, 0.0));
    }
    let header = Rect::from_min_size(r.min, Vec2::new(w, header_h));
    cx.with_clip(r, |cx| {
        draw_tabbox_header(m, id, cx, header, true);
        if let Some(a) = active {
            let slide = match &mut m.nodes[a].kind { Kind::TabboxTab(t) => cx.anim_value(&t.slide), _ => 0.0 };
            let cy = header.max.y + cx.px(1.0) + cx.px(7.0) + cx.px(10.0) * slide;
            let content_clip = Rect::from_min_max(Pos2::new(r.min.x, header.max.y + cx.px(1.0)), r.max);
            cx.with_clip(content_clip, |cx| {
                let _ = crate::elements::pass_children(m, &children, cx, x + cx.px(7.0), cy, inner_w, gap, false);
            });
        }
    });
    if !in_groupbox || in_float {
        cx.outline(r, corners, 0.0);
    }
    if let Kind::Tabbox(t) = &mut m.nodes[id].kind {
        t.pop.header = header;
    }
    popout::handle_header(m, id, cx, header, r);
    height
}

