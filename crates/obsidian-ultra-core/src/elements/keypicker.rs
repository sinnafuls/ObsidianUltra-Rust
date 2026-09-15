//! Key picker addon (keybinds), keybind-frame rows and runtime key handling.

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, Layer};
use crate::handles::info::KeyPickerInfo;
use crate::input::{Input, Key, Modifier, MouseButton};
use crate::node::*;
use crate::tween::info as tw;
use crate::types::*;
use crate::ui::{Model, Pending};

use super::menu::{self, MenuCorners, MenuSpec};

/// Picker square height on Label/Toggle hosts.
const SQUARE: f32 = 18.0;
/// Button-host holder height (the square stretches to it).
const BUTTON_ROW: f32 = 21.0;
/// Button-host square width cap before the text starts sliding.
const MAX_BUTTON_W: f32 = 85.0;
/// Mode menu width.
const MENU_W: f32 = 62.0;
/// Keybind-frame row height.
const ROW_H: f32 = 16.0;
/// Marquee: design px per second and pauses.
const MARQUEE_SPEED: f32 = 25.0;
const MARQUEE_MIN: f32 = 0.35;
const MARQUEE_PAUSE: f32 = 1.5;

pub(crate) fn create(m: &mut Model, host: NodeId, idx: &str, info: KeyPickerInfo) -> NodeId {
    let host_is_button = matches!(m.nodes[host].kind, Kind::Button(_));
    let mut mode = info.mode;
    let mut modes = info.modes.clone();
    let mut sync = info.sync_toggle_state;
    if host_is_button || mode == KeyMode::Press {
        mode = KeyMode::Press;
        modes = vec![KeyMode::Press];
        sync = false;
    } else if sync {
        modes = vec![KeyMode::Toggle, KeyMode::Hold];
        if !modes.contains(&mode) {
            mode = KeyMode::Toggle;
        }
    }
    if !modes.contains(&mode) {
        modes.push(mode);
    }
    let value = KeyBind { key: info.default, modifiers: info.default_modifiers.clone(), mode };
    let data = KeyPickerData {
        text: info.text,
        value: value.clone(),
        modes,
        sync_toggle_state: sync,
        no_ui: info.no_ui,
        wait_for_callback: info.wait_for_callback,
        blacklisted: info.blacklisted,
        blacklisted_modifiers: info.blacklisted_modifiers,
        whitelisted: info.whitelisted,
        whitelisted_modifiers: info.whitelisted_modifiers,
        toggled: false,
        picking: Picking::default(),
        changed: info.changed.into_iter().collect(),
        callback: info.callback.into_iter().collect(),
        clicked: info.clicked.into_iter().collect(),
        menu: MenuState::NONE,
        host,
        host_is_button,
        default: value,
        marquee_start: 0.0,
        marquee_text: String::new(),
        hold_active: false,
    };
    let id = m.insert(Kind::KeyPicker(Box::new(data)), Some(host));
    match &mut m.nodes[host].kind {
        Kind::Toggle(t) => t.addons.push(id),
        Kind::Label(l) => l.addons.push(id),
        Kind::Button(b) => b.addons.push(id),
        _ => {}
    }
    m.key_pickers.push(id);
    m.register_option(idx, id);
    id
}

fn data(m: &Model, id: NodeId) -> Option<&KeyPickerData> {
    match m.nodes.get(id).map(|n| &n.kind) {
        Some(Kind::KeyPicker(k)) if !m.nodes[id].destroyed => Some(k),
        _ => None,
    }
}

fn data_mut(m: &mut Model, id: NodeId) -> Option<&mut KeyPickerData> {
    match m.nodes.get_mut(id).map(|n| &mut n.kind) {
        Some(Kind::KeyPicker(k)) => Some(k),
        _ => None,
    }
}

/// The host Toggle's `(value, disabled)` when the host is a Toggle.
fn host_toggle(m: &Model, host: NodeId) -> Option<(bool, bool)> {
    match m.nodes.get(host).map(|n| &n.kind) {
        Some(Kind::Toggle(t)) => Some((t.value, t.disabled)),
        _ => None,
    }
}
pub(crate) fn set_value(m: &mut Model, id: NodeId, value: KeyBind) {
    let Some(k) = data_mut(m, id) else { return };
    k.value.key = match value.key {
        KeyValue::Key(Key::Unknown) => KeyValue::Unknown,
        other => other,
    };
    k.value.modifiers = value.modifiers;
    if k.modes.contains(&value.mode) {
        k.value.mode = value.mode;
    }
    let out = k.value.clone();
    let menu_open = k.menu.open;
    if menu_open {
        let mut state = k.menu;
        menu::close(m, id, 0, &mut state);
        if let Some(k) = data_mut(m, id) {
            k.menu = state;
        }
    }
    update(m, id);
    m.pending.push(Pending::Key(id, out));
}
pub(crate) fn state(m: &Model, id: NodeId) -> bool {
    match &m.nodes[id].kind {
        Kind::KeyPicker(k) => match k.value.mode {
            KeyMode::Always => true,
            KeyMode::Hold => k.hold_active,
            _ => k.toggled,
        },
        _ => false,
    }
}

/// `KeyPicker:Update` minus the display part: mirror the state into a synced host Toggle.
fn update(m: &mut Model, id: NodeId) {
    let Some(k) = data(m, id) else { return };
    if k.no_ui {
        return;
    }
    let (host, sync, mode) = (k.host, k.sync_toggle_state, k.value.mode);
    let Some((host_value, host_disabled)) = host_toggle(m, host) else { return };
    if mode == KeyMode::Toggle && host_disabled {
        return;
    }
    let st = state(m, id);
    if sync && host_value != st {
        super::toggle::set_value(m, host, st);
    }
}
pub(crate) fn do_click(m: &mut Model, id: NodeId) {
    let Some(k) = data_mut(m, id) else { return };
    if k.picking.active {
        return;
    }
    let press = k.value.mode == KeyMode::Press;
    if press {
        if k.toggled && k.wait_for_callback {
            return;
        }
        k.toggled = true;
    }
    let toggled = k.toggled;
    if press {
        k.toggled = false;
    }
    m.pending.push(Pending::KeyClick(id, toggled));
}

// ----- runtime input -----------------------------------------------------------------------

fn input_down(input: &Input, kv: KeyValue) -> bool {
    match kv {
        KeyValue::Mouse(b) => input.is_down(b),
        KeyValue::Key(k) => input.key_down(k),
        _ => false,
    }
}

fn input_pressed(input: &Input, kv: KeyValue) -> bool {
    match kv {
        KeyValue::Mouse(b) => input.pressed[b.index()],
        KeyValue::Key(k) => input.key_pressed(k),
        _ => false,
    }
}

/// True when the list is empty.
fn modifiers_held(input: &Input, mods: &[Modifier]) -> bool {
    mods.iter().all(|m| input.key_down(m.key()))
}

/// `IsValidInput` for one candidate while picking.
fn valid_pick(k: &KeyPickerData, kv: KeyValue) -> bool {
    match kv {
        KeyValue::Key(Key::Escape) => true,
        KeyValue::Key(key) => match key.modifier() {
            Some(md) => {
                (k.whitelisted_modifiers.is_empty() || k.whitelisted_modifiers.contains(&md)) && !k.blacklisted_modifiers.contains(&md)
            }
            None => (k.whitelisted.is_empty() || k.whitelisted.contains(&kv)) && !k.blacklisted.contains(&kv),
        },
        KeyValue::Mouse(_) => (k.whitelisted.is_empty() || k.whitelisted.contains(&kv)) && !k.blacklisted.contains(&kv),
        _ => true,
    }
}

/// First valid input that began this frame (keyboard first, then mouse buttons), excluding `skip`.
fn first_pick_input(k: &KeyPickerData, input: &Input, skip: Option<Key>) -> Option<KeyValue> {
    if input.key_pressed(Key::Escape) {
        return Some(KeyValue::Key(Key::Escape));
    }
    for key in &input.keys_pressed {
        if Some(*key) == skip {
            continue;
        }
        let kv = KeyValue::Key(*key);
        if valid_pick(k, kv) {
            return Some(kv);
        }
    }
    for b in MouseButton::ALL {
        if input.pressed[b.index()] {
            let kv = KeyValue::Mouse(b);
            if valid_pick(k, kv) {
                return Some(kv);
            }
        }
    }
    None
}

/// Start the picking flow (left click on the square).
fn start_picking(m: &mut Model, id: NodeId) {
    if m.picking {
        return;
    }
    let Some(k) = data_mut(m, id) else { return };
    if k.picking.active {
        return;
    }
    k.picking = Picking { active: true, ..Default::default() };
    m.picking = true;
}

fn stop_picking(m: &mut Model, id: NodeId) {
    if let Some(k) = data_mut(m, id) {
        k.picking = Picking::default();
    }
    m.picking = false;
    update(m, id);
}

/// Bind `kv` with the accumulated modifiers, then wait for its release.
fn resolve_pick(m: &mut Model, id: NodeId, kv: KeyValue) {
    let Some(k) = data(m, id) else { return };
    let host = k.host;
    let mode = k.value.mode;
    let key = match kv {
        KeyValue::Key(Key::Escape) => KeyValue::None,
        KeyValue::Key(Key::Unknown) => KeyValue::Unknown,
        other => other,
    };
    let modifiers = if matches!(key, KeyValue::None | KeyValue::Unknown) { Vec::new() } else { k.picking.modifiers.clone() };
    let toggled = host_toggle(m, host).map(|(v, _)| v).unwrap_or(false);
    if let Some(k) = data_mut(m, id) {
        k.toggled = toggled;
        k.picking.current_modifier = None;
        k.picking.waiting_release = Some(kv);
    }
    set_value(m, id, KeyBind { key, modifiers, mode });
}

/// One frame of the picking state machine for picker `id`.
fn advance_picking(m: &mut Model, id: NodeId, input: &Input) {
    if input.text_edit_focused {
        stop_picking(m, id);
        return;
    }
    let Some(k) = data(m, id) else {
        m.picking = false;
        return;
    };
    if let Some(wait) = k.picking.waiting_release {
        if !input_down(input, wait) {
            stop_picking(m, id);
        }
        return;
    }
    match k.picking.current_modifier {
        Some(cm) => {
            // Wait for the modifier's release (-> it becomes the key) or another valid input.
            if let Some(next) = first_pick_input(k, input, Some(cm.key())) {
                if let Some(k) = data_mut(m, id) {
                    if !k.picking.modifiers.contains(&cm) {
                        k.picking.modifiers.push(cm);
                    }
                }
                match next {
                    KeyValue::Key(Key::Escape) => resolve_pick(m, id, next),
                    KeyValue::Key(key) if key.modifier().is_some() => {
                        if let Some(k) = data_mut(m, id) {
                            k.picking.current_modifier = key.modifier();
                        }
                    }
                    _ => resolve_pick(m, id, next),
                }
            } else if !input.key_down(cm.key()) || input.keys_released.contains(&cm.key()) {
                resolve_pick(m, id, KeyValue::Key(cm.key()));
            }
        }
        None => {
            if let Some(first) = first_pick_input(k, input, None) {
                match first {
                    KeyValue::Key(Key::Escape) => resolve_pick(m, id, first),
                    KeyValue::Key(key) if key.modifier().is_some() => {
                        if let Some(k) = data_mut(m, id) {
                            k.picking.current_modifier = key.modifier();
                        }
                    }
                    _ => resolve_pick(m, id, first),
                }
            }
        }
    }
}

/// Global per-frame key handling for every key picker (runs even while the UI is hidden).
pub(crate) fn runtime_input(m: &mut Model, cx: &mut Cx) {
    let input: &Input = cx.fi.input;
    let ids = m.key_pickers.clone();
    let mut any_picking = false;
    for id in ids.iter().copied() {
        let Some(k) = data(m, id) else { continue };
        if k.picking.active {
            advance_picking(m, id, input);
            if data(m, id).map(|k| k.picking.active).unwrap_or(false) {
                any_picking = true;
            }
            cx.animate();
        }
    }
    m.picking = any_picking;

    let ui_open = m.open;
    let text_focus = input.text_edit_focused;
    for id in ids {
        let Some(k) = data(m, id) else { continue };
        if k.picking.active {
            continue;
        }
        let (mode, key, mods) = (k.value.mode, k.value.key, k.value.modifiers.clone());
        let is_mouse = matches!(key, KeyValue::Mouse(_));
        let bound = !matches!(key, KeyValue::None | KeyValue::Unknown);
        let held_mods = modifiers_held(input, &mods);

        // Hold state is evaluated live every frame (`GetState` -> `IsKeyDown`).
        let hold = mode == KeyMode::Hold && bound && held_mods && !text_focus && !(is_mouse && ui_open) && input_down(input, key);
        let prev_hold = k.hold_active;
        if let Some(k) = data_mut(m, id) {
            k.hold_active = hold;
        }
        if hold != prev_hold {
            update(m, id);
        }

        if mode == KeyMode::Always || !bound || any_picking || text_focus || (is_mouse && ui_open) {
            continue;
        }
        if held_mods && input_pressed(input, key) {
            match mode {
                KeyMode::Toggle => {
                    if let Some(k) = data_mut(m, id) {
                        k.toggled = !k.toggled;
                    }
                    do_click(m, id);
                }
                KeyMode::Press => do_click(m, id),
                _ => {}
            }
            update(m, id);
        }
    }
}

// ----- picker square ----------------------------------------------------------------------

/// Text shown in the square: the bind, or the in-progress pick.
fn square_text(k: &KeyPickerData) -> String {
    if !k.picking.active {
        return k.value.display_value();
    }
    match k.picking.current_modifier {
        Some(cm) => {
            let mut s = String::new();
            for md in &k.picking.modifiers {
                s.push_str(md.short_name());
                s.push_str(" + ");
            }
            s.push_str(cm.short_name());
            s.push_str(" + ...");
            s
        }
        None => "...".to_owned(),
    }
}

/// Draw the 18×18 picker square right-aligned at `right_x`, centered on `cy`. Returns the width used.
pub(crate) fn pass_addon(m: &mut Model, id: NodeId, cx: &mut Cx, right_x: f32, cy: f32) -> f32 {
    let Some(k) = data(m, id) else { return 0.0 };
    let (host_is_button, menu_open, modes, mode, picking) = (k.host_is_button, k.menu.open, k.modes.clone(), k.value.mode, k.picking.active);
    let text = square_text(k);
    let text_w = cx.text_size(&text, 14.0, None, false).x;
    let h = cx.px(if host_is_button { BUTTON_ROW } else { SQUARE });
    let natural_w = text_w + cx.px(9.0);
    let w = if host_is_button { natural_w.min(cx.px(MAX_BUTTON_W)) } else { natural_w.max(cx.px(SQUARE)) };
    let r = Rect::from_min_size(Pos2::new((right_x - w).floor(), (cy - h / 2.0).floor()), Vec2::new(w.ceil(), h));
    m.nodes[id].rect = r;

    let hovered = cx.hovered(r);
    let hv = {
        let n = &mut m.nodes[id];
        let mut a = n.hover;
        let v = cx.tween(&mut a, if hovered { 1.0 } else { 0.0 }, tw::HOVER);
        n.hover = a;
        v
    };
    let r2 = cx.r2();
    let corners = if menu_open { Corners { nw: r2, ne: 0.0, sw: r2, se: 0.0 } } else { Corners::same(r2) };
    cx.rect(r, corners, cx.col(cx.sch.main, 0.0));
    cx.stroke(r, corners, 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Inside);
    let color = cx.col(cx.sch.font_color, 0.4 * (1.0 - hv));

    // Text: centered, or sliding when it overflows a button-host square.
    let overflow = natural_w - w - cx.px(4.5);
    if host_is_button && overflow > 0.0 {
        let start = match data_mut(m, id) {
            Some(k) => {
                if k.marquee_text != text {
                    k.marquee_text = text.clone();
                    k.marquee_start = cx.time;
                }
                k.marquee_start
            }
            None => cx.time,
        };
        let dur = (overflow / cx.m.s / MARQUEE_SPEED).max(MARQUEE_MIN);
        let cycle = 2.0 * (dur + MARQUEE_PAUSE);
        let t = ((cx.time - start) as f32).rem_euclid(cycle);
        let pad = cx.px(4.5);
        let x_off = if t < dur {
            pad + (-overflow - pad) * (t / dur)
        } else if t < dur + MARQUEE_PAUSE {
            -overflow
        } else if t < 2.0 * dur + MARQUEE_PAUSE {
            -overflow + (pad + overflow) * ((t - dur - MARQUEE_PAUSE) / dur)
        } else {
            pad
        };
        cx.animate();
        let tr = Rect::from_min_size(Pos2::new(r.min.x + x_off, r.min.y), Vec2::new(natural_w, r.height()));
        cx.with_clip(r, |cx| {
            cx.text(tr, &text, 14.0, color, Align::Min, false, false);
        });
    } else {
        cx.with_clip(r, |cx| {
            cx.text(r, &text, 14.0, color, Align::Center, false, false);
        });
    }

    if cx.clicked(r) && !picking {
        start_picking(m, id);
    } else if cx.right_clicked(r) {
        let mut st = match data(m, id) {
            Some(k) => k.menu,
            None => return w,
        };
        menu::toggle(m, id, 0, &mut st);
        if let Some(k) = data_mut(m, id) {
            k.menu = st;
        }
    }

    // Mode menu.
    let row_h = cx.px(if host_is_button { BUTTON_ROW } else if modes.len() == 1 { 18.0 } else { 19.0 });
    let corners = if modes.len() == 1 { MenuCorners::NoLeft } else { MenuCorners::NoTopLeft };
    let anim = if m.settings.animations.key_picker { Some(tw::KEYPICKER) } else { None };
    let mut st = match data(m, id) {
        Some(k) => k.menu,
        None => return w,
    };
    let spec = MenuSpec {
        holder: r,
        width: cx.px(MENU_W),
        offset: Vec2::new(r.width() + cx.px(1.5), cx.px(0.5)),
        height: row_h * modes.len() as f32,
        anim,
    };
    let menu_rect = menu::begin(m, id, 0, cx, &mut st, spec);
    if let Some(k) = data_mut(m, id) {
        k.menu = st;
    }
    if let Some(mr) = menu_rect {
        let open = st.open;
        cx.with_layer(Layer::Menus, mr, false, |cx| {
            menu::chrome(cx, mr, corners);
            let mut picked = None;
            for (i, md) in modes.iter().enumerate() {
                let row = Rect::from_min_size(Pos2::new(mr.min.x, mr.min.y + row_h * i as f32), Vec2::new(mr.width(), row_h));
                let selected = *md == mode;
                let hovered = open && cx.hovered(row);
                let bg_t = if selected { 0.0 } else if hovered { 0.7 } else { 1.0 };
                let text_t = if selected { 0.0 } else if hovered { 0.1 } else { 0.5 };
                if bg_t < 0.999 {
                    let last = i + 1 == modes.len();
                    let rc = if modes.len() == 1 {
                        Corners::no_left(r2)
                    } else if i == 0 {
                        Corners { nw: 0.0, ne: r2, sw: 0.0, se: 0.0 }
                    } else if last {
                        Corners { nw: 0.0, ne: 0.0, sw: r2, se: r2 }
                    } else {
                        Corners::ZERO
                    };
                    cx.rect(row, rc, cx.col(cx.sch.main, bg_t));
                }
                cx.text(row, md.label(), 14.0, cx.col(cx.sch.font_color, text_t), Align::Center, false, false);
                if hovered && cx.pressed(MouseButton::Left) {
                    picked = Some(*md);
                }
            }
            if let Some(md) = picked {
                if let Some(k) = data_mut(m, id) {
                    k.value.mode = md;
                    let mut st = k.menu;
                    menu::close(m, id, 0, &mut st);
                    if let Some(k) = data_mut(m, id) {
                        k.menu = st;
                    }
                }
                update(m, id);
            }
        });
    }
    w
}

// ----- keybind frame rows -----------------------------------------------------------------

/// Draw the keybind-frame rows at `(x, y)` with width `w`. Returns the height used (0 when no rows).
pub(crate) fn pass_keybind_rows(m: &mut Model, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let ids = m.key_pickers.clone();
    let show_frame_toggles = m.settings.show_toggle_frame_in_keybinds;
    let row_h = cx.px(ROW_H);
    let mut yy = y;
    for id in ids {
        let Some(k) = data(m, id) else { continue };
        if k.no_ui {
            continue;
        }
        let (mode, host) = (k.value.mode, k.host);
        if mode == KeyMode::Toggle && host_toggle(m, host).map(|(_, d)| d).unwrap_or(false) {
            continue;
        }
        let Some(k) = data(m, id) else { continue };
        let label = format!("[{}] {} ({})", k.value.display_value(), k.text, mode.label());
        let st = state(m, id);
        let show_toggle = show_frame_toggles && mode == KeyMode::Toggle;
        let row = Rect::from_min_size(Pos2::new(x, yy), Vec2::new(w, row_h));
        let mut label_x = x;
        if show_toggle {
            let bx = Rect::from_min_size(Pos2::new(x, yy + cx.px(1.0)), Vec2::splat(cx.px(14.0)));
            cx.rect(bx, cx.r2(), cx.col(cx.sch.main, 0.0));
            cx.stroke(bx, cx.r2(), 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Inside);
            if st {
                cx.icon(&IconRef::Lucide("check".into()), bx.shrink(cx.px(2.0)), cx.col(cx.sch.font_color, 0.0));
            }
            label_x = x + cx.px(22.0);
        }
        let lr = Rect::from_min_max(Pos2::new(label_x, yy), Pos2::new(x + w, yy + row_h));
        let color = cx.col(cx.sch.font_color, if st { 0.0 } else { 0.5 });
        cx.with_clip(lr, |cx| {
            cx.text(lr, &label, 14.0, color, Align::Min, false, false);
        });
        if show_toggle && cx.clicked(row) {
            if let Some(k) = data_mut(m, id) {
                k.toggled = !k.toggled;
            }
            do_click(m, id);
            update(m, id);
        }
        yy += row_h;
    }
    yy - y
}

/// Widest keybind-frame row (text + checkbox inset), in points; 0 when no rows are shown.
pub(crate) fn keybind_rows_width(m: &mut Model, cx: &mut Cx) -> f32 {
    let ids = m.key_pickers.clone();
    let show_frame_toggles = m.settings.show_toggle_frame_in_keybinds;
    let mut widest = 0.0f32;
    for id in ids {
        let Some(k) = data(m, id) else { continue };
        if k.no_ui {
            continue;
        }
        let (mode, host) = (k.value.mode, k.host);
        if mode == KeyMode::Toggle && host_toggle(m, host).map(|(_, d)| d).unwrap_or(false) {
            continue;
        }
        let Some(k) = data(m, id) else { continue };
        let label = format!("[{}] {} ({})", k.value.display_value(), k.text, mode.label());
        let inset = if show_frame_toggles && mode == KeyMode::Toggle { cx.px(22.0) } else { 0.0 };
        let w = cx.text_size(&label, 14.0, None, false).x + inset;
        widest = widest.max(w);
    }
    widest
}
