//! Frame orchestration and window-level mutations.

pub(crate) mod boxes;
pub(crate) mod chrome;
pub(crate) mod popout;
pub(crate) mod tabs;

use epaint::{Pos2, Rect, Vec2};

use crate::frame::{Cx, Layer};
use crate::handles::info::WindowInfo;
use crate::handles::Window;
use crate::input::{Key, MouseButton};
use crate::node::*;
use crate::tween::{info as tw, Anim};
use crate::ui::{Model, Ui};

impl Ui {
    /// Create the main window. Panics if a window already exists.
    pub fn create_window(&self, info: WindowInfo) -> Window {
        let id = self.mutate(|m| create_window(m, info));
        Window { ui: self.clone(), id }
    }
}

pub(crate) fn create_window(m: &mut Model, info: WindowInfo) -> NodeId {
    assert!(m.window.is_none(), "window already created");
    let screen = m.screen;
    let max = Vec2::new((screen.width() - 64.0).max(480.0), (screen.height() - 64.0).max(360.0));
    let min_size = Vec2::new(info.min_container_width.min(max.x).max(1.0), 360.0f32.min(max.y));
    let size = Vec2::new(info.size.x.clamp(min_size.x, max.x), info.size.y.clamp(min_size.y, max.y));
    let pos = if info.center { Pos2::new(screen.center().x - size.x / 2.0, screen.center().y - size.y / 2.0) } else { info.position };
    let initial_left = (size.x * 0.3).ceil();
    m.corner_radius = info.corner_radius.min(20.0);
    m.scheme.font = info.font.clone();
    m.settings.toggle_keybind = info.toggle_keybind;
    m.settings.global_search = info.global_search;
    m.settings.fuzzy_search = info.fuzzy_search;
    m.settings.search_values = info.search_values;
    m.settings.show_custom_cursor = info.show_custom_cursor;
    m.settings.animations = info.animations;
    m.settings.notify_side = info.notify_side;
    m.settings.unlock_mouse_while_open = info.unlock_mouse_while_open;
    if info.background_image.is_some() {
        m.scheme.background_image = info.background_image.clone();
    }
    let data = WindowData {
        placed: false,
        title: info.title.clone(),
        pos,
        size,
        min_size,
        sidebar_width: initial_left,
        last_expanded_width: initial_left,
        compact: false,
        minimized: false,
        mini_pos: pos,
        mini_subtitle: info.minimized_subtitle.clone(),
        mini_subtitle_explicit: !info.minimized_subtitle.is_empty(),
        mini_labels: Vec::new(),
        tabs: Vec::new(),
        active_tab: None,
        footer: info.footer.clone(),
        footer_copied: None,
        glow: GlowConfig { enabled: info.glow, ..Default::default() },
        snapping: info.snapping,
        snap_distance: info.snap_distance.max(0.0),
        snap_margin: info.snap_margin.max(0.0),
        search_text: String::new(),
        search_sent: String::new(),
        search_focus: crate::frame::FocusRequest::None,
        search_focus_anim: Anim::new(0.0),
        dialogs: Vec::new(),
        always_on_top: info.always_on_top,
        background_image: info.background_image.clone(),
        corner_radius: m.corner_radius,
        tab_transition_time: info.tab_transition_time.max(0.0),
        tab_swipe_offset: info.tab_swipe_offset.max(1.0),
        tab_swipe_from: info.tab_swipe_from,
        grabber_hover: Anim::new(0.0),
        minimize_hover: Anim::new(0.0),
        bell_hover: Anim::new(0.0),
        mini_bell_hover: Anim::new(0.0),
        snap_guide_x: None,
        snap_guide_y: None,
        info,
    };
    let auto_show = data.info.auto_show;
    let auto_minimize = data.info.auto_minimize && data.info.minimizable;
    let compacted = data.info.enable_compacting && data.info.sidebar_compacted;
    let compact_w = data.info.sidebar_compact_width.max(48.0);
    let id = m.insert(Kind::Window(Box::new(data)), None);
    m.window = Some(id);
    if compacted {
        set_sidebar_width(m, id, compact_w);
    }
    if auto_minimize {
        set_minimized(m, id, Some(true));
    }
    if auto_show && m.loading.is_none() {
        toggle(m, Some(true));
    }
    id
}

/// `Window:SetSidebarWidth` + `ApplyCompact`.
pub(crate) fn set_sidebar_width(m: &mut Model, win: NodeId, width: f32) {
    let Kind::Window(w) = &mut m.nodes[win].kind else { return };
    let max = w.size.x - w.info.min_container_width - 1.0;
    let width = width.clamp(48.0, max.max(48.0));
    w.sidebar_width = width;
    if w.info.enable_compacting {
        let compact_w = w.info.sidebar_compact_width.max(48.0);
        w.compact = if w.info.disable_compacting_snap { width <= w.info.compact_width_activation.max(48.0) } else { (width - compact_w).abs() < 0.5 };
    } else {
        w.compact = false;
    }
    if !w.compact {
        w.last_expanded_width = width;
    }
    let compact = w.compact;
    let active = w.active_tab;
    if !compact {
        if let Some(t) = active {
            if let Kind::Tab(td) = &mut m.nodes[t].kind {
                if !td.sub_tabs.is_empty() {
                    td.sidebar_expanded = true;
                }
            }
        }
    }
}
pub(crate) fn set_minimized(m: &mut Model, win: NodeId, value: Option<bool>) {
    let Kind::Window(w) = &mut m.nodes[win].kind else { return };
    if !w.info.minimizable {
        return;
    }
    let v = value.unwrap_or(!w.minimized);
    if v == w.minimized {
        return;
    }
    w.minimized = v;
    if v {
        w.mini_pos = w.pos;
    } else {
        w.pos = w.mini_pos;
    }
}

/// `Window:Toggle` / `Library:Toggle`.
pub(crate) fn toggle(m: &mut Model, value: Option<bool>) {
    if m.fading {
        return;
    }
    if m.loading.is_some() {
        // A loading screen owns the screen: the window cannot open.
        if value == Some(true) || (value.is_none() && !m.open) {
            return;
        }
    }
    let new = value.unwrap_or(!m.open);
    if new == m.open {
        return;
    }
    m.open = new;
    if m.settings.animations.toggle_window && m.window.is_some() {
        m.window_fade.set(if new { 1.0 } else { 0.0 }, m.time, tw::WINDOW.0, tw::WINDOW.1);
    } else {
        m.window_fade.snap(if new { 1.0 } else { 0.0 });
    }
    if !new {
        m.tooltip.visible = false;
        m.tooltip.node = None;
        close_all_menus(m);
        m.capture = None;
        // Release any hosted text-field focus.
        if let Some(w) = m.window {
            if let Kind::Window(wd) = &mut m.nodes[w].kind {
                wd.search_focus = crate::frame::FocusRequest::Release;
            }
        }
    }
}

pub(crate) fn close_all_menus(m: &mut Model) {
    let ids: Vec<NodeId> = m.nodes.iter().filter(|(_, n)| matches!(n.kind, Kind::Dropdown(_) | Kind::Priority(_) | Kind::KeyPicker(_) | Kind::ColorPicker(_))).map(|(id, _)| id).collect();
    for id in ids {
        match &mut m.nodes[id].kind {
            Kind::Dropdown(d) => {
                d.menu.open = false;
                d.menu.anim.snap(0.0);
            }
            Kind::Priority(p) => {
                p.menu.open = false;
                p.menu.anim.snap(0.0);
            }
            Kind::KeyPicker(k) => {
                k.menu.open = false;
                k.menu.anim.snap(0.0);
            }
            Kind::ColorPicker(c) => {
                c.menu.open = false;
                c.menu.anim.snap(0.0);
                c.ctx_menu.open = false;
                c.ctx_menu.anim.snap(0.0);
            }
            _ => {}
        }
    }
    m.current_menu = None;
}

/// Close whichever menu is currently open.
pub(crate) fn close_current_menu(m: &mut Model) {
    let Some((id, sub)) = m.current_menu else { return };
    if let Some(n) = m.nodes.get_mut(id) {
        match &mut n.kind {
            Kind::Dropdown(d) => d.menu.open = false,
            Kind::Priority(p) => p.menu.open = false,
            Kind::KeyPicker(k) => k.menu.open = false,
            Kind::ColorPicker(c) => {
                if sub == 0 {
                    c.menu.open = false;
                } else {
                    c.ctx_menu.open = false;
                }
            }
            _ => {}
        }
    }
    m.current_menu = None;
}

/// Open a menu, closing any other (Lua single-menu rule).
pub(crate) fn open_menu(m: &mut Model, id: NodeId, sub: u8) {
    if m.current_menu != Some((id, sub)) {
        close_current_menu(m);
    }
    m.current_menu = Some((id, sub));
}

/// Called after a handle's `set_visible`.
pub(crate) fn after_visibility_change(m: &mut Model, id: NodeId, visible: bool) {
    let kind = m.nodes.get(id).map(|n| std::mem::discriminant(&n.kind));
    let _ = kind;
    match m.nodes.get(id).map(|n| &n.kind) {
        Some(Kind::Tab(_)) | Some(Kind::KeyTab(_)) => {
            if !visible && m.window.and_then(|w| match &m.nodes[w].kind { Kind::Window(wd) => wd.active_tab, _ => None }) == Some(id) {
                tabs::hide(m, id);
            }
        }
        Some(Kind::SubTab(_)) => {
            if !visible {
                tabs::subtab_hidden(m, id);
            }
        }
        Some(Kind::Groupbox(_)) | Some(Kind::Tabbox(_)) => {
            if visible && m.searching {
                let t = m.search_text.clone();
                crate::overlays::search::update_search(m, &t);
            }
        }
        _ => {}
    }
}

/// Destroy any node through its handle.
pub(crate) fn destroy_node(m: &mut Model, id: NodeId) {
    if !m.alive(id) {
        return;
    }
    match &m.nodes[id].kind {
        Kind::Groupbox(g) if g.pop.popped => popout::set_popped_out(m, id, false, None),
        Kind::Tabbox(t) if t.pop.popped => popout::set_popped_out(m, id, false, None),
        Kind::Loading(_) => {
            crate::overlays::loading::destroy(m, id);
            return;
        }
        Kind::Notification(_) => {
            crate::overlays::notify::destroy(m, id);
            return;
        }
        Kind::Dialog(_) => {
            crate::overlays::dialog::dismiss(m, id);
            return;
        }
        _ => {}
    }
    let is_tab = matches!(m.nodes[id].kind, Kind::Tab(_) | Kind::KeyTab(_));
    if is_tab {
        tabs::hide(m, id);
    }
    if let Kind::SubTab(_) = m.nodes[id].kind {
        tabs::subtab_hidden(m, id);
    }
    m.destroy_subtree(id);
    if is_tab {
        // Show the first remaining tab if none is active.
        if let Some(w) = m.window {
            let (active, first) = match &m.nodes[w].kind {
                Kind::Window(wd) => (wd.active_tab, wd.tabs.first().copied()),
                _ => (None, None),
            };
            if active.is_none() {
                if let Some(f) = first {
                    tabs::show(m, f);
                }
            }
        }
    }
}

/// Which overlay layer (from the previous frame) is under the pointer, if any.
fn top_overlay_layer(m: &Model, pointer: Option<Pos2>) -> Option<Layer> {
    let p = pointer?;
    m.overlays.iter().filter(|(_, r)| r.contains(p)).map(|(l, _)| *l).max()
}

/// The topmost float (by z order) under the pointer, from previous-frame rects.
pub(crate) fn top_float_at(m: &Model, p: Pos2) -> Option<NodeId> {
    m.floats.iter().rev().copied().find(|id| m.nodes.get(*id).map(|n| n.shown() && n.rect.contains(p)).unwrap_or(false))
}

pub(crate) fn frame(m: &mut Model, cx: &mut Cx) {
    let pointer = cx.pointer();
    let top = top_overlay_layer(m, pointer);
    m.prev_overlays = std::mem::take(&mut m.overlays);
    cx.out.wants_pointer = false;
    cx.out.wants_keyboard = cx.fi.input.text_edit_focused;

    // Window fade.
    let fade = cx.anim_value(&m.window_fade);
    m.fading = m.window_fade.active(cx.time);
    cx.fade = fade;

    global_keys(m, cx);
    crate::elements::keypicker::runtime_input(m, cx);

    let blocked_window = top.map(|l| l > Layer::Window).unwrap_or(false);
    let visible = m.open || m.fading || m.window_fade.value(cx.time) > 0.001;
    if let Some(win) = m.window {
        if visible {
            cx.with_layer(Layer::Window, m.screen, blocked_window, |cx| chrome::pass(m, win, cx));
        }
    }
    if let Some(l) = m.loading {
        cx.fade = 1.0;
        cx.with_layer(Layer::Window, m.screen, blocked_window, |cx| crate::overlays::loading::pass(m, l, cx));
    }
    cx.fade = 1.0;

    // Floats: topmost float under the pointer gets input; menus above block all floats.
    let blocked_floats = top.map(|l| l > Layer::Floats).unwrap_or(false);
    let top_float = pointer.and_then(|p| top_float_at(m, p));
    let floats = m.floats.clone();
    for id in floats {
        if !m.alive(id) {
            continue;
        }
        let blocked = blocked_floats || (top_float.is_some() && top_float != Some(id));
        cx.with_layer(Layer::Floats, m.screen, blocked, |cx| crate::overlays::floats::pass(m, id, cx));
    }
    crate::overlays::history::pass(m, cx, blocked_floats);
    crate::overlays::notify::pass(m, cx);
    crate::overlays::tooltip::pass(m, cx);
    crate::overlays::cursor::pass(m, cx);

    // Release pointer capture when the button is up.
    if let Some(c) = m.capture {
        let uses_right = c.sub == cap::SLIDER && c.data[3] == 1.0;
        let down = if uses_right { cx.down(MouseButton::Right) } else { cx.down(MouseButton::Left) };
        if !down {
            m.capture = None;
        }
    }
    if m.capture.is_some() {
        cx.out.wants_pointer = true;
        cx.animate();
    }
    if let Some(p) = pointer {
        if m.overlays.iter().any(|(_, r)| r.contains(p)) {
            cx.out.wants_pointer = true;
        }
    }
    cx.out.interactive_rects = m.overlays.iter().map(|(_, r)| *r).collect();
}

/// Is `rect` (on `layer`) covered by a higher overlay from the previous frame?
pub(crate) fn covered(m: &Model, layer: Layer, rect: Rect) -> bool {
    m.prev_overlays.iter().any(|(l, r)| *l > layer && r.intersects(rect))
}

fn global_keys(m: &mut Model, cx: &mut Cx) {
    let input = cx.fi.input;
    if input.keys_pressed.is_empty() {
        return;
    }
    let text_focused = input.text_edit_focused;
    if input.key_pressed(Key::Escape) {
        if text_focused {
            return;
        }
        if let Some(d) = m.active_dialog {
            let dismissable = matches!(&m.nodes[d].kind, Kind::Dialog(dd) if dd.outside_click_dismiss);
            if dismissable {
                crate::overlays::dialog::dismiss(m, d);
                return;
            }
        }
        if m.current_menu.is_some() {
            close_current_menu(m);
            return;
        }
        if let Some(e) = m.active_expanded {
            match &mut m.nodes[e].kind {
                Kind::Dropdown(_) => crate::elements::dropdown::set_expanded(m, e, false),
                Kind::Priority(_) => crate::elements::priority::set_expanded(m, e, false),
                _ => {}
            }
            return;
        }
        return;
    }
    if text_focused {
        return;
    }
    if !m.picking {
        if let Some(k) = m.settings.toggle_keybind {
            if input.key_pressed(k) {
                toggle(m, None);
            }
        }
        if let Some(k) = m.settings.notification_history_keybind {
            if input.key_pressed(k) {
                crate::overlays::history::set_visible(m, !m.history_open);
            }
        }
    }
    let Some(win) = m.window else { return };
    let (minimize_key, minimizable, search_key, search_enabled) = match &m.nodes[win].kind {
        Kind::Window(w) => (
            w.info.minimize_keybind,
            w.info.minimizable,
            w.info.search_keybind,
            !w.info.disable_search && !w.info.disable_search_keybind,
        ),
        _ => return,
    };
    if m.open && minimizable {
        if let Some(k) = minimize_key {
            if input.key_pressed(k) {
                set_minimized(m, win, None);
            }
        }
    }
    if m.open && search_enabled && input.ctrl_down() && input.key_pressed(search_key) {
        if let Kind::Window(w) = &mut m.nodes[win].kind {
            w.search_focus = crate::frame::FocusRequest::Take;
        }
    }
}

/// Record an overlay rect for next-frame pointer blocking.
pub(crate) fn record_overlay(m: &mut Model, layer: Layer, rect: Rect) {
    if rect.is_positive() {
        m.overlays.push((layer, rect));
    }
}
