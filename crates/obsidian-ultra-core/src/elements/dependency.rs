//! Dependency box (inline) and dependency groupbox (sibling box).

use epaint::{Pos2, Rect, Vec2};

use crate::draw::Corners;
use crate::frame::Cx;
use crate::node::*;
use crate::types::*;
use crate::ui::Model;

/// `groupbox == false` -> inline DepBox child of `parent`; `true` -> sibling DepGroupbox in the same column.
pub(crate) fn create(m: &mut Model, parent: NodeId, groupbox: bool) -> NodeId {
    let side = match &m.nodes[parent].kind {
        Kind::Groupbox(g) => g.side,
        _ => Side::Left,
    };
    let data = DepData { side, deps: Vec::new(), satisfied: false, height: 0.0 };
    if groupbox {
        // Sibling of the groupbox in its owner's column.
        let owner = m.nodes[parent].parent.unwrap_or(parent);
        let id = m.insert(Kind::DepGroupbox(data), Some(owner));
        m.nodes[id].visible = false;
        match &mut m.nodes[owner].kind {
            Kind::Tab(t) => t.boxes.push(id),
            Kind::SubTab(t) => t.boxes.push(id),
            _ => m.nodes[owner].children.push(id),
        }
        m.dep_boxes.push(id);
        id
    } else {
        let id = m.insert(Kind::DepBox(data), Some(parent));
        m.nodes[id].visible = false;
        m.add_child(parent, id);
        m.dep_boxes.push(id);
        id
    }
}

/// Is one condition currently met? Destroyed or unknown nodes never satisfy.
fn satisfied(m: &Model, dep: &Dependency) -> bool {
    match dep {
        Dependency::Toggle(t, expected) => match m.nodes.get(*t) {
            Some(n) if !n.destroyed => matches!(&n.kind, Kind::Toggle(td) if td.value == *expected),
            _ => false,
        },
        Dependency::Dropdown(d, v) => match m.nodes.get(*d) {
            Some(n) if !n.destroyed => matches!(&n.kind, Kind::Dropdown(dd) if dd.value.contains(v)),
            _ => false,
        },
    }
}
pub(crate) fn update(m: &mut Model, id: NodeId, cancel_search: bool) {
    let Some(node) = m.nodes.get(id) else { return };
    if node.destroyed {
        return;
    }
    let all = match &node.kind {
        Kind::DepBox(d) | Kind::DepGroupbox(d) => d.deps.iter().all(|dep| satisfied(m, dep)),
        _ => return,
    };
    let node = &mut m.nodes[id];
    if let Kind::DepBox(d) | Kind::DepGroupbox(d) = &mut node.kind {
        d.satisfied = all;
    }
    node.visible = all;
    if all && m.searching && !cancel_search {
        let text = m.search_text.clone();
        crate::overlays::search::update_search(m, &text);
    }
}

fn store_height(m: &mut Model, id: NodeId, h: f32) {
    if let Kind::DepBox(d) | Kind::DepGroupbox(d) = &mut m.nodes[id].kind {
        d.height = h;
    }
}

pub(crate) fn dep_box_height(m: &mut Model, id: NodeId, cx: &mut Cx, w: f32) -> f32 {
    let Some(n) = m.nodes.get(id) else { return 0.0 };
    if !n.shown() {
        return 0.0;
    }
    let children = n.children.clone();
    let h = crate::elements::measure_children(m, &children, cx, w, cx.px(8.0));
    store_height(m, id, h);
    h
}

pub(crate) fn pass_dep_box(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let Some(n) = m.nodes.get(id) else { return 0.0 };
    if !n.shown() {
        return 0.0;
    }
    let children = n.children.clone();
    let key_tab = cx.key_tab;
    let h = crate::elements::pass_children(m, &children, cx, x, y, w, cx.px(8.0), key_tab);
    m.nodes[id].rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    store_height(m, id, h);
    h
}

pub(crate) fn pass_dep_groupbox(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let Some(n) = m.nodes.get(id) else { return 0.0 };
    if !n.shown() || !matches!(n.kind, Kind::DepGroupbox(_)) {
        return 0.0;
    }
    let children = n.children.clone();
    let gap = cx.px(8.0);
    let pad = cx.px(7.0);
    let inner_w = w - cx.px(14.0);
    let content = crate::elements::measure_children(m, &children, cx, inner_w, gap);
    let h = content + cx.px(18.0);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    m.nodes[id].rect = r;
    let corners = Corners::same(cx.r());
    cx.rect(r, corners, cx.col(cx.sch.background, 0.0));
    cx.with_clip(r, |cx| {
        let _ = crate::elements::pass_children(m, &children, cx, x + pad, y + pad, inner_w, gap, false);
    });
    cx.outline(r, corners, 0.0);
    store_height(m, id, h);
    h
}
