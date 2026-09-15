//! Search filtering (`UpdateSearch` / `ApplySearchToTab` / `ResetTab` / `CheckDepbox`).
//!
//! Only `Node::search_visible` is touched here; user visibility (`Node::visible`) is
//! left alone and combined by `Node::shown()` at draw time.

use crate::node::*;
use crate::ui::Model;
pub(crate) fn update_search(m: &mut Model, text: &str) {
    m.search_text = text.to_owned();
    let global = m.settings.global_search;
    let Some(win) = m.window else {
        m.searching = false;
        m.last_search_tab = None;
        return;
    };
    let (tabs, active) = match &m.nodes[win].kind {
        Kind::Window(w) => (w.tabs.clone(), w.active_tab),
        _ => return,
    };
    // Every non-key tab (global) or the tab searched last time.
    let non_key: Vec<NodeId> = tabs.into_iter().filter(|t| m.alive(*t) && matches!(m.nodes[*t].kind, Kind::Tab(_))).collect();
    if global {
        for t in &non_key {
            reset_subtree(m, *t);
        }
    } else if let Some(last) = m.last_search_tab {
        if m.alive(last) {
            reset_subtree(m, last);
        }
    }

    let search = crate::search::normalize(text);
    if search.is_empty() {
        reset_all(m);
        m.searching = false;
        m.last_search_tab = None;
        return;
    }
    let active = active.filter(|a| m.alive(*a));
    if !global && !matches!(active.map(|a| &m.nodes[a].kind), Some(Kind::Tab(_))) {
        m.searching = false;
        m.last_search_tab = None;
        return;
    }
    m.searching = true;

    let to_search: Vec<NodeId> = if global { non_key } else { active.into_iter().collect() };
    let mut first_visible = None;
    let mut active_has_visible = false;
    for t in to_search {
        if apply_tab(m, t, &search) {
            if first_visible.is_none() && m.nodes[t].visible {
                first_visible = Some(t);
            }
            if Some(t) == active {
                active_has_visible = true;
            }
        }
    }

    if global {
        if !active_has_visible {
            if let Some(first) = first_visible {
                if active != Some(first) {
                    // `show` re-applies the search for the new active tab.
                    crate::window::tabs::show(m, first);
                }
            }
        }
        m.last_search_tab = None;
    } else {
        m.last_search_tab = active;
    }
}

/// Restore every node's `search_visible` flag.
pub(crate) fn reset_all(m: &mut Model) {
    for (_, n) in m.nodes.iter_mut() {
        n.search_visible = true;
    }
}

/// Every node directly owned by `id` (children plus kind-specific lists).
fn owned(n: &Node) -> Vec<NodeId> {
    let mut v = n.children.clone();
    match &n.kind {
        Kind::Tab(t) => v.extend(t.boxes.iter().chain(t.sub_tabs.iter()).chain(t.banners.iter())),
        Kind::SubTab(t) => v.extend(t.boxes.iter()),
        Kind::Tabbox(t) => v.extend(t.tabs.iter()),
        Kind::Button(b) => v.extend(b.sub),
        _ => {}
    }
    v
}

/// `ResetTab` / `RestoreDepbox`: show everything under `id` again.
fn reset_subtree(m: &mut Model, id: NodeId) {
    let Some(n) = m.nodes.get_mut(id) else { return };
    n.search_visible = true;
    for c in owned(n) {
        reset_subtree(m, c);
    }
}

// ----- matching ------------------------------------------------------------------------------

/// `TryFuzzyMatch`, honouring `settings.fuzzy_search` (off -> plain substring).
fn text_matches(m: &Model, text: &str, search: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if m.settings.fuzzy_search {
        crate::search::matches(text, search)
    } else {
        text.to_lowercase().contains(search)
    }
}

fn opt_matches(m: &Model, text: Option<&str>, search: &str) -> bool {
    text.map(|t| text_matches(m, t, search)).unwrap_or(false)
}

/// `ElementInfo.Text` truthiness: elements without a text field never match a search.
fn has_text(m: &Model, id: NodeId) -> bool {
    match &m.nodes[id].kind {
        Kind::Dropdown(d) => d.text.is_some(),
        Kind::Priority(p) => p.text.is_some(),
        _ => crate::overlays::element_text(m, id).is_some(),
    }
}

fn tips(n: &Node) -> Option<&Tips> {
    match &n.kind {
        Kind::Button(b) => Some(&b.tips),
        Kind::Toggle(t) => Some(&t.tips),
        Kind::Input(i) => Some(&i.tips),
        Kind::Slider(s) => Some(&s.tips),
        Kind::Dropdown(d) => Some(&d.tips),
        Kind::Priority(p) => Some(&p.tips),
        _ => None,
    }
}
fn matches_element(m: &Model, id: NodeId, search: &str, force: bool) -> bool {
    if force {
        return true;
    }
    if let Some(t) = crate::overlays::element_text(m, id) {
        if text_matches(m, &t, search) {
            return true;
        }
    }
    let n = &m.nodes[id];
    if let Some(t) = tips(n) {
        if opt_matches(m, t.tooltip.as_deref(), search) || opt_matches(m, t.disabled_tooltip.as_deref(), search) {
            return true;
        }
    }
    if m.settings.search_values {
        let hit = match &n.kind {
            Kind::Dropdown(d) => value_hit(m, d.values.iter().map(|e| (e.value.as_str(), e.display_text())), search),
            Kind::Priority(p) => value_hit(m, p.values.iter().map(|v| (v.as_str(), v.as_str())), search),
            _ => false,
        };
        if hit {
            return true;
        }
    }
    false
}

fn value_hit<'a>(m: &Model, values: impl IntoIterator<Item = (&'a str, &'a str)>, search: &str) -> bool {
    if m.settings.fuzzy_search {
        crate::search::matches_values(values, search)
    } else {
        values.into_iter().take(crate::search::VALUE_CAP).any(|(v, d)| text_matches(m, d, search) || text_matches(m, v, search))
    }
}

// ----- application ---------------------------------------------------------------------------

/// Filter the element list of a container (groupbox body, tabbox tab, dependency box).
/// `force` reveals everything (the container or its tab matched); `dividers` is false inside
/// dependency boxes where dividers are always hidden while searching. Returns the number of
/// visible elements.
fn filter_container(m: &mut Model, id: NodeId, search: &str, force: bool, dividers: bool) -> usize {
    let children = m.nodes[id].children.clone();
    let mut count = 0usize;
    for c in children {
        if !m.alive(c) {
            continue;
        }
        match &m.nodes[c].kind {
            Kind::Divider(_) => {
                m.nodes[c].search_visible = dividers && force;
            }
            Kind::Button(b) if b.sub.map(|s| m.alive(s)).unwrap_or(false) => {
                let sub = b.sub.unwrap();
                let main_v = matches_element(m, c, search, force);
                let sub_v = matches_element(m, sub, search, force);
                m.nodes[c].search_visible = main_v;
                m.nodes[sub].search_visible = sub_v;
                if (main_v && m.nodes[c].visible) || (sub_v && m.nodes[sub].visible) {
                    count += 1;
                }
            }
            Kind::DepBox(_) => {
                // Lua skips boxes whose dependencies are not met (`not Depbox.Visible`).
                if m.nodes[c].visible {
                    let inner = filter_container(m, c, search, force, false);
                    m.nodes[c].search_visible = inner > 0;
                    count += inner;
                }
            }
            Kind::Tabbox(_) => {
                // Nested tabbox: Lua registers it on the tab; here it lives in the groupbox body.
                if m.nodes[c].visible && apply_tabbox(m, c, search, force) {
                    count += 1;
                }
            }
            _ => {
                let v = has_text(m, c) && matches_element(m, c, search, force);
                m.nodes[c].search_visible = v;
                if v && m.nodes[c].visible {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Filter a tabbox: each tab is a container; a tab whose name matches reveals its elements.
/// Auto-switches to the first tab with hits when the active one has none.
fn apply_tabbox(m: &mut Model, id: NodeId, search: &str, force: bool) -> bool {
    let (tabs, active) = match &m.nodes[id].kind {
        Kind::Tabbox(t) => (t.tabs.clone(), t.active),
        _ => return false,
    };
    let mut counts: Vec<(NodeId, usize)> = Vec::with_capacity(tabs.len());
    for t in tabs {
        if !m.alive(t) {
            continue;
        }
        let name = match &m.nodes[t].kind {
            Kind::TabboxTab(tt) => tt.name.clone(),
            _ => continue,
        };
        let tab_force = force || opt_matches(m, name.as_deref(), search);
        let count = filter_container(m, t, search, tab_force, true);
        m.nodes[t].search_visible = count > 0;
        counts.push((t, count));
    }
    let visible_tabs = counts.iter().filter(|(_, c)| *c > 0).count();
    let active_count = active.and_then(|a| counts.iter().find(|(t, _)| *t == a)).map(|(_, c)| *c).unwrap_or(0);
    if active.is_some() && active_count == 0 {
        if let Some((first, _)) = counts.iter().find(|(t, c)| *c > 0 && m.nodes[*t].visible) {
            crate::window::boxes::show_tabbox_tab(m, *first);
        }
    }
    m.nodes[id].search_visible = visible_tabs > 0;
    visible_tabs > 0
}

/// `ApplySearchToTab(Tab, Search) -> HasVisible` for a tab or a sidebar sub tab.
fn apply_tab(m: &mut Model, id: NodeId, search: &str) -> bool {
    let (name, description, boxes, sub_tabs, active_sub) = match &m.nodes[id].kind {
        Kind::Tab(t) => (t.name.clone(), t.description.clone(), t.boxes.clone(), t.sub_tabs.clone(), t.active_sub_tab),
        Kind::SubTab(s) => (s.name.clone(), None, s.boxes.clone(), Vec::new(), None),
        _ => return false,
    };
    let tab_matches = text_matches(m, &name, search) || opt_matches(m, description.as_deref(), search);
    let mut has_visible = false;

    for b in boxes {
        if !m.alive(b) || !m.nodes[b].visible {
            continue;
        }
        match &m.nodes[b].kind {
            Kind::Groupbox(g) => {
                let force = tab_matches || text_matches(m, &g.name, search) || opt_matches(m, g.description.as_deref(), search);
                let count = filter_container(m, b, search, force, true);
                m.nodes[b].search_visible = count > 0;
                has_visible |= count > 0;
            }
            Kind::DepGroupbox(_) => {
                let count = filter_container(m, b, search, tab_matches, true);
                m.nodes[b].search_visible = count > 0;
                has_visible |= count > 0;
            }
            Kind::Tabbox(_) => {
                has_visible |= apply_tabbox(m, b, search, tab_matches);
            }
            _ => {}
        }
    }

    // Sidebar sub tabs hold their own boxes.
    let mut first_visible_sub = None;
    let mut active_sub_visible = true;
    for st in sub_tabs {
        if !m.alive(st) {
            continue;
        }
        let sub_name = match &m.nodes[st].kind {
            Kind::SubTab(s) => s.name.clone(),
            _ => continue,
        };
        let visible = if text_matches(m, &sub_name, search) {
            // The sub tab itself is the hit: show all of its contents.
            reset_subtree(m, st);
            true
        } else {
            apply_tab(m, st, search)
        };
        m.nodes[st].search_visible = visible;
        if visible {
            has_visible = true;
            if first_visible_sub.is_none() && m.nodes[st].visible {
                first_visible_sub = Some(st);
            }
        }
        if Some(st) == active_sub {
            active_sub_visible = visible;
        }
    }
    if active_sub.is_some() && !active_sub_visible {
        if let Some(first) = first_visible_sub {
            crate::window::tabs::show(m, first);
        }
    }

    has_visible
}
