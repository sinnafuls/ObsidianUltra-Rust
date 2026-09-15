//! Behavior tests driven through the headless harness.

use std::sync::Arc;

use parking_lot::Mutex;

use obsidian_ultra_core::testing::Harness;
use obsidian_ultra_core::*;

fn window(ui: &Ui) -> Window {
    ui.create_window(WindowInfo { title: "T".into(), ..Default::default() })
}

#[test]
fn toggle_click_flips_value_and_fires_listener_once() {
    let ui = Ui::new();
    let w = window(&ui);
    let tab = w.add_tab(TabInfo::new("Main"));
    let g = tab.add_groupbox(GroupboxInfo::left("G"));
    let t = g.add_toggle("A", ToggleInfo::new("A"));
    let hits = Arc::new(Mutex::new(Vec::new()));
    let h2 = hits.clone();
    t.on_changed(move |v| h2.lock().push(v));
    let mut h = Harness::new(ui.clone());
    h.settle(2);
    let c = h.center(t.node_id());
    h.click(c);
    assert!(t.value());
    assert_eq!(*hits.lock(), vec![true]);
    assert!(ui.toggle_by("A").is_some());
}

#[test]
fn disabled_toggle_ignores_set_value_and_clicks() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let t = g.add_toggle("D", ToggleInfo { disabled: true, ..ToggleInfo::new("D") });
    t.set_value(true);
    assert!(!t.value());
    let mut h = Harness::new(ui);
    h.settle(2);
    let c = h.center(t.node_id());
    h.click(c);
    assert!(!t.value());
}

#[test]
fn listener_reentrancy_does_not_deadlock() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let a = g.add_toggle("A", ToggleInfo::new("A"));
    let b = g.add_toggle("B", ToggleInfo::new("B"));
    let b2 = b.clone();
    let ui2 = ui.clone();
    a.on_changed(move |v| {
        b2.set_value(v);
        ui2.notify_text("changed", 1.0);
    });
    a.set_value(true);
    assert!(b.value());
    assert_eq!(ui.notification_history().len(), 1);
}

#[test]
fn slider_clamps_and_rounds() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let s = g.add_slider("S", SliderInfo::new("S", 0.0, 100.0, 50.0));
    s.set_value(150.0);
    assert_eq!(s.value(), 100.0);
    let s2 = g.add_slider("S2", SliderInfo::new("S2", 0.0, 100.0, 0.0).rounding(2));
    let mut h = Harness::new(ui);
    h.settle(2);
    // Drag to 12.345% of the bar: value rounds to 2 decimals.
    let r = h.rect(s2.node_id());
    let bar_y = r.max.y - 4.0 - 8.0;
    let x = r.min.x + r.width() * 0.12345;
    h.press(Pos2::new(x, bar_y), MouseButton::Left);
    h.frame(1.0 / 60.0);
    h.release(MouseButton::Left);
    h.frame(1.0 / 60.0);
    let v = s2.value();
    assert!((v * 100.0).fract().abs() < 1e-6, "rounded to 2 decimals: {v}");
    assert!((v - 12.35).abs() < 0.6, "near the pressed position: {v}");
}

#[test]
fn input_pipeline_rules() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let empty = g.add_input("E", InputInfo { allow_empty: false, ..InputInfo::new("E") });
    empty.set_value("   ");
    assert_eq!(empty.value(), "---");
    let num = g.add_input("N", InputInfo { numeric: true, default: "12".into(), ..InputInfo::new("N") });
    num.set_value("abc");
    assert_eq!(num.value(), "12");
    num.set_value("13.5");
    assert_eq!(num.value(), "13.5");
    let verified = g.add_input("V", InputInfo { verify_value: Some(Box::new(|s: &str| s.starts_with('x'))), empty_reset: "nope".into(), ..InputInfo::new("V") });
    verified.set_value("hello");
    assert_eq!(verified.value(), "nope");
    verified.set_value("xyz");
    assert_eq!(verified.value(), "xyz");
    let capped = g.add_input("M", InputInfo { max_length: Some(3), ..InputInfo::new("M") });
    capped.set_value("abcdef");
    assert_eq!(capped.value(), "abc");
}

#[test]
fn typing_into_hosted_field_commits_on_change_and_finished_waits_for_submit() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let live = g.add_input("L", InputInfo::new("L"));
    let finished = g.add_input("F", InputInfo { finished: true, ..InputInfo::new("F") });
    let mut h = Harness::new(ui);
    h.settle(2);
    h.type_into(live.node_id(), 0, "typed", false);
    assert_eq!(live.value(), "typed");
    h.type_into(finished.node_id(), 0, "pending", false);
    assert_eq!(finished.value(), "");
    h.type_into(finished.node_id(), 0, "done", true);
    assert_eq!(finished.value(), "done");
}

#[test]
fn button_click_and_double_click_confirm() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let count = Arc::new(Mutex::new(0));
    let c1 = count.clone();
    let b = g.add_button(ButtonInfo::new("B", move || *c1.lock() += 1));
    let c2 = count.clone();
    let d = g.add_button(ButtonInfo { double_click: true, ..ButtonInfo::new("D", move || *c2.lock() += 10) });
    let mut h = Harness::new(ui);
    h.settle(2);
    let bc = h.center(b.node_id());
    h.click(bc);
    assert_eq!(*count.lock(), 1);
    let dc = h.center(d.node_id());
    h.click(dc);
    assert_eq!(*count.lock(), 1, "first click only asks for confirmation");
    h.click(dc);
    assert_eq!(*count.lock(), 11);
}

#[test]
fn tabs_switch_on_click_and_window_toggle_keybind() {
    let ui = Ui::new();
    let w = window(&ui);
    let t1 = w.add_tab(TabInfo::new("One"));
    let t2 = w.add_tab(TabInfo::new("Two"));
    let g1 = t1.add_groupbox(GroupboxInfo::left("G1"));
    let g2 = t2.add_groupbox(GroupboxInfo::left("G2"));
    let mut h = Harness::new(ui.clone());
    h.settle(2);
    assert!(h.rect(g1.node_id()).is_positive());
    assert!(!h.rect(g2.node_id()).is_positive(), "inactive tab content is not laid out");
    let c = h.center(t2.node_id());
    h.click(c);
    h.settle(2);
    assert!(h.rect(g2.node_id()).is_positive());
    assert!(ui.is_open());
    h.tap(Key::RightControl);
    assert!(!ui.is_open());
    h.tap(Key::RightControl);
    assert!(ui.is_open());
}

#[test]
fn groupbox_collapse_hides_children() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let t = g.add_toggle("A", ToggleInfo::new("A"));
    let mut h = Harness::new(ui);
    h.settle(2);
    let expanded_h = h.rect(g.node_id()).height();
    g.set_collapsed(true);
    h.settle(3);
    assert!(h.rect(g.node_id()).height() < expanded_h);
    assert!(g.is_collapsed());
    let _ = t;
}

#[test]
fn destroy_removes_from_registries() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let t = g.add_toggle("A", ToggleInfo::new("A"));
    let s = g.add_slider("S", SliderInfo::new("S", 0.0, 1.0, 0.0));
    assert!(ui.option("S").is_some());
    t.destroy();
    s.destroy();
    assert!(t.is_destroyed());
    assert!(ui.toggle_by("A").is_none());
    assert!(ui.option("S").is_none());
}

#[test]
fn unload_runs_callbacks_and_stops_frames() {
    let ui = Ui::new();
    let flag = Arc::new(Mutex::new(false));
    let f2 = flag.clone();
    ui.on_unload(move || *f2.lock() = true);
    let _w = window(&ui);
    ui.unload();
    assert!(*flag.lock());
    assert!(ui.is_unloaded());
    let mut h = Harness::new(ui);
    let out = h.frame(0.016);
    assert!(out.layers.iter().all(|l| l.is_empty()));
}

#[test]
fn dependency_box_follows_toggle_and_dropdown() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let master = g.add_toggle("Master", ToggleInfo::new("Master"));
    let mode = g.add_dropdown("Mode", DropdownInfo::new("Mode", ["A", "B"]).default_value("A"));
    let dep = g.add_dependency_box();
    let inner = dep.add_toggle("Inner", ToggleInfo::new("Inner"));
    dep.set_dependencies(vec![Dependency::Toggle(master.clone(), true), Dependency::Dropdown(mode.clone(), "B".into())]);
    let mut h = Harness::new(ui.clone());
    h.settle(2);
    assert!(!ui.node_shown(inner.node_id()), "hidden while master is off");
    master.set_value(true);
    h.settle(2);
    assert!(!ui.node_shown(inner.node_id()), "still hidden: dropdown is A");
    mode.set_value(DropdownValue::Single(Some("B".into())));
    h.settle(2);
    assert!(ui.node_shown(inner.node_id()));
    assert!(h.rect(inner.node_id()).is_positive());
    master.set_value(false);
    h.settle(2);
    assert!(!ui.node_shown(inner.node_id()));
}

#[test]
fn search_filters_elements_groupboxes_and_dropdown_values() {
    let ui = Ui::new();
    let w = window(&ui);
    let tab = w.add_tab(TabInfo::new("Main"));
    let combat = tab.add_groupbox(GroupboxInfo::left("Combat"));
    let aim = combat.add_toggle("Aim", ToggleInfo::new("Aimbot"));
    let div = combat.add_divider(DividerInfo::text("Sep"));
    let esp = combat.add_toggle("Esp", ToggleInfo::new("ESP"));
    let misc = tab.add_groupbox(GroupboxInfo::right("Misc"));
    let bone = misc.add_dropdown("Bone", DropdownInfo::new("Bone", ["Head", "Torso"]));
    let other = misc.add_toggle("Other", ToggleInfo::new("Other"));
    let mut h = Harness::new(ui.clone());
    h.settle(2);

    ui.update_search("aim");
    h.settle(1);
    assert!(ui.node_shown(aim.node_id()));
    assert!(!ui.node_shown(esp.node_id()));
    assert!(!ui.node_shown(div.node_id()), "dividers hide unless the groupbox matched");
    assert!(!ui.node_shown(misc.node_id()), "groupbox with no hits hides");

    ui.update_search("torso");
    h.settle(1);
    assert!(ui.node_shown(bone.node_id()), "dropdown value name reveals the dropdown");
    assert!(ui.node_shown(misc.node_id()));
    assert!(!ui.node_shown(other.node_id()));

    ui.update_search("combat");
    h.settle(1);
    assert!(ui.node_shown(aim.node_id()) && ui.node_shown(esp.node_id()) && ui.node_shown(div.node_id()), "groupbox name reveals everything inside");

    ui.update_search("");
    h.settle(1);
    assert!(ui.node_shown(other.node_id()) && ui.node_shown(misc.node_id()) && ui.node_shown(div.node_id()));
}

#[test]
fn key_picker_states_and_picking_flow() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let t = g.add_toggle("T", ToggleInfo::new("T"));
    let always = t.add_key_picker("Always", KeyPickerInfo::key("A", Key::G).mode(KeyMode::Always));
    assert!(always.state());
    let hold = g.add_label(LabelInfo::new("Hold")).add_key_picker("Hold", KeyPickerInfo::key("H", Key::H).mode(KeyMode::Hold));
    let pick = g.add_label(LabelInfo::new("Pick")).add_key_picker("Pick", KeyPickerInfo { blacklisted: vec![KeyValue::Key(Key::G)], ..KeyPickerInfo::new("P", KeyValue::None) });
    let changed = Arc::new(Mutex::new(Vec::new()));
    let c2 = changed.clone();
    pick.on_changed(move |k| c2.lock().push(k.clone()));
    let mut h = Harness::new(ui.clone());
    ui.toggle(Some(false));
    h.settle(2);
    // Hold: true only while the key is down (works while the UI is closed).
    h.key_down(Key::H);
    h.frame(0.016);
    assert!(hold.state());
    h.key_up(Key::H);
    h.frame(0.016);
    assert!(!hold.state());
    // Picking: click the square, hold LCtrl, press F -> LCtrl + F.
    ui.toggle(Some(true));
    h.settle(2);
    let sq = h.center(pick.node_id());
    h.click(sq);
    h.tap(Key::G); // blacklisted: ignored
    assert_eq!(pick.value().key, KeyValue::None);
    h.key_down(Key::LeftControl);
    h.frame(0.016);
    h.key_down(Key::F);
    h.frame(0.016);
    h.key_up(Key::F);
    h.key_up(Key::LeftControl);
    h.settle(2);
    let v = pick.value();
    assert_eq!(v.key, KeyValue::Key(Key::F));
    assert_eq!(v.modifiers, vec![Modifier::LCtrl]);
    assert_eq!(v.display_value(), "LCtrl + F");
    assert_eq!(changed.lock().len(), 1);
    // Runtime: LCtrl+F flips the Toggle-mode picker and fires on_click.
    let clicks = Arc::new(Mutex::new(0));
    let cl = clicks.clone();
    pick.on_click(move |_| *cl.lock() += 1);
    ui.toggle(Some(false));
    h.settle(1);
    h.tap(Key::F);
    assert_eq!(*clicks.lock(), 0, "F alone does not match LCtrl + F");
    h.key_down(Key::LeftControl);
    h.frame(0.016);
    h.tap(Key::F);
    h.key_up(Key::LeftControl);
    h.frame(0.016);
    assert_eq!(*clicks.lock(), 1);
    assert!(pick.state());
}

#[test]
fn color_picker_set_value_and_listener() {
    let ui = Ui::new();
    let w = window(&ui);
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    let cp = g.add_toggle("T", ToggleInfo::new("T")).add_color_picker("C", ColorPickerInfo::new(Color32::from_rgb(255, 0, 0)).with_alpha(0.25));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    cp.on_changed(move |c| s2.lock().push(c));
    assert_eq!(cp.value().color, Color32::from_rgb(255, 0, 0));
    assert!((cp.value().transparency - 0.25).abs() < 1e-6);
    cp.set_value(Rgba { color: Color32::from_rgb(10, 20, 30), transparency: 0.5 });
    assert_eq!(cp.value().color, Color32::from_rgb(10, 20, 30));
    assert_eq!(seen.lock().len(), 1);
    assert!(matches!(ui.option("C").map(|o| o.value()), Some(Value::Color(_))));
}

#[test]
fn groupbox_pops_out_after_hold_and_docks_back_near_placeholder() {
    let ui = Ui::new();
    let w = window(&ui);
    let tab = w.add_tab(TabInfo::new("Main"));
    let g = tab.add_groupbox(GroupboxInfo::left("Pop"));
    g.add_toggle("A", ToggleInfo::new("A"));
    let mut h = Harness::new(ui.clone());
    h.settle(2);
    let r = h.rect(g.node_id());
    let header = Pos2::new(r.min.x + 40.0, r.min.y + 12.0);
    // Press, hold 0.2s, move 60px right -> detached float.
    h.press(header, MouseButton::Left);
    h.frame(0.016);
    h.frame(0.2);
    h.move_to(header + Vec2::new(60.0, 0.0));
    h.frame(0.016);
    h.move_to(header + Vec2::new(620.0, 0.0));
    h.frame(0.016);
    assert!(g.is_popped_out());
    h.release(MouseButton::Left);
    h.settle(2);
    assert!(g.is_popped_out(), "dropped outside the window stays popped out");
    let float_rect = h.rect(g.node_id());
    assert!(float_rect.min.x > r.min.x + 500.0);
    // Drag back near the placeholder -> docks.
    let fh = Pos2::new(float_rect.min.x + 40.0, float_rect.min.y + 12.0);
    h.press(fh, MouseButton::Left);
    h.frame(0.016);
    h.frame(0.2);
    h.move_to(fh - Vec2::new(620.0, 0.0));
    h.frame(0.016);
    h.release(MouseButton::Left);
    h.settle(2);
    assert!(!g.is_popped_out());
    g.set_popped_out(true);
    assert!(g.is_popped_out());
    g.set_popped_out(false);
    assert!(!g.is_popped_out());
}

#[test]
fn minimize_swaps_frames_and_labels() {
    let ui = Ui::new();
    let w = window(&ui);
    let tab = w.add_tab(TabInfo::new("Main"));
    let _g = tab.add_groupbox(GroupboxInfo::left("G"));
    let mut h = Harness::new(ui.clone());
    h.settle(2);
    let full = h.rect(w.node_id());
    let l = w.add_minimized_label("status: idle");
    w.set_minimized(Some(true));
    h.settle(2);
    let mini = h.rect(w.node_id());
    assert!(w.is_minimized());
    assert!(mini.width() < full.width() && mini.height() < full.height());
    l.set_text("status: busy");
    w.toggle_minimized();
    h.settle(2);
    assert!(!w.is_minimized());
    assert_eq!(h.rect(w.node_id()).size(), full.size());
}

#[test]
fn notifications_expire_and_persist_and_history_tracks_unread() {
    let ui = Ui::new();
    let _w = window(&ui);
    let mut h = Harness::new(ui.clone());
    h.settle(2);
    let short = ui.notify(NotifyInfo::new("A", "short").time(0.5));
    let sticky = ui.notify(NotifyInfo { persist: true, ..NotifyInfo::new("B", "sticky") });
    h.settle(3);
    assert!(h.rect(short.node_id()).is_positive());
    for _ in 0..12 {
        h.frame(0.1);
    }
    assert!(short.is_destroyed(), "expired after time + slide");
    assert!(!sticky.is_destroyed());
    assert!(h.rect(sticky.node_id()).is_positive());
    assert_eq!(ui.notification_history().len(), 2);
    assert!(!ui.is_notification_history_open());
    ui.set_notification_history_visible(true);
    h.settle(2);
    assert!(ui.is_notification_history_open());
    ui.clear_notification_history();
    assert!(ui.notification_history().is_empty());
}

#[test]
fn dialog_buttons_fire_and_dismiss() {
    let ui = Ui::new();
    let w = window(&ui);
    w.add_tab(TabInfo::new("Main"));
    let fired = Arc::new(Mutex::new(false));
    let f2 = fired.clone();
    let d = w.add_dialog("Q", DialogInfo { title: "Q".into(), description: "Sure?".into(), ..Default::default() });
    d.add_footer_button("ok", FooterButtonInfo { title: Some("OK".into()), callback: Some(Box::new(move |_d| *f2.lock() = true)), ..Default::default() });
    let mut h = Harness::new(ui.clone());
    h.settle(3);
    let dr = h.rect(d.node_id());
    assert!(dr.is_positive());
    // Button sits bottom-right inside the frame: click near there.
    let ok = Pos2::new(dr.max.x - 40.0, dr.max.y - 28.0);
    h.click(ok);
    h.settle(10);
    assert!(*fired.lock(), "footer callback ran");
    assert!(d.is_destroyed(), "auto_dismiss removes the dialog");
    // Escape dismisses a fresh dialog.
    let d2 = w.add_dialog("E", DialogInfo::default());
    h.settle(2);
    h.tap(Key::Escape);
    h.settle(10);
    assert!(d2.is_destroyed());
}

#[test]
fn unsupported_screen_matches_lists() {
    let ui = Ui::new();
    assert!(ui.create_unsupported_screen(UnsupportedInfo { executor: "Synapse Z".into(), supported: Some(vec!["synapse".into()]), ..Default::default() }).is_none());
    let ui = Ui::new();
    let s = ui.create_unsupported_screen(UnsupportedInfo { executor: "Foo".into(), unsupported: Some(vec!["foo".into()]), ..Default::default() });
    assert!(s.is_some());
    assert!(ui.option("UnsupportedLanguage").is_some());
    let mut h = Harness::new(ui.clone());
    h.settle(2);
    assert!(h.rect(s.unwrap().window.node_id()).is_positive());
}

#[test]
fn resize_handle_and_sidebar_grabber() {
    let ui = Ui::new();
    let w = window(&ui);
    w.add_tab(TabInfo::new("Main").icon("house"));
    let mut h = Harness::new(ui.clone());
    h.settle(2);
    let r = h.rect(w.node_id());
    assert_eq!(r.size(), Vec2::new(720.0, 600.0));
    // Drag the bottom-right handle: grows by (100, 50); shrinking below 480×360 clamps.
    let handle = Pos2::new(r.max.x - 8.0, r.max.y - 8.0);
    h.press(handle, MouseButton::Left);
    h.frame(0.016);
    h.move_to(handle + Vec2::new(100.0, 50.0));
    h.frame(0.016);
    h.release(MouseButton::Left);
    h.frame(0.016);
    assert_eq!(w.size_position().0, Vec2::new(820.0, 650.0));
    let r = h.rect(w.node_id());
    let handle = Pos2::new(r.max.x - 8.0, r.max.y - 8.0);
    h.press(handle, MouseButton::Left);
    h.frame(0.016);
    h.move_to(handle - Vec2::new(600.0, 600.0));
    h.frame(0.016);
    h.release(MouseButton::Left);
    h.frame(0.016);
    assert_eq!(w.size_position().0, Vec2::new(256.0, 360.0), "clamped to the minimum size (Lua MinSize = (MinContainerWidth, 360))");
    // Sidebar divider: drag far left -> compact (48px); drag right -> expanded again.
    w.set_size_position(Some(Vec2::new(720.0, 600.0)), None);
    h.settle(2);
    let r = h.rect(w.node_id());
    let sidebar = w.sidebar_width();
    let grab = Pos2::new(r.min.x + sidebar, r.min.y + 200.0);
    h.press(grab, MouseButton::Left);
    h.frame(0.016);
    h.move_to(grab - Vec2::new(120.0, 0.0));
    h.frame(0.016);
    h.release(MouseButton::Left);
    h.frame(0.016);
    assert!(w.is_sidebar_compacted());
    assert_eq!(w.sidebar_width(), 48.0);
    let grab = Pos2::new(r.min.x + 48.0, r.min.y + 200.0);
    h.press(grab, MouseButton::Left);
    h.frame(0.016);
    h.move_to(grab + Vec2::new(120.0, 0.0));
    h.frame(0.016);
    h.release(MouseButton::Left);
    h.frame(0.016);
    assert!(!w.is_sidebar_compacted());
    assert!(w.sidebar_width() >= 128.0);
    w.set_compact(true);
    assert!(w.is_sidebar_compacted());
}
