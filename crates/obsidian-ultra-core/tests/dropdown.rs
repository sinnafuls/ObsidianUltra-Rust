//! Dropdown / priority dropdown behavior: selection contracts and the frame-driven list,
//! expanded panel and drag-to-rank interactions.

use std::sync::Arc;

use epaint::{Pos2, Rect, Vec2};
use parking_lot::Mutex;

use obsidian_ultra_core::frame::Layer;
use obsidian_ultra_core::input::{Key, MouseButton};
use obsidian_ultra_core::testing::Harness;
use obsidian_ultra_core::*;

fn setup() -> (Ui, Window, Groupbox) {
    let ui = Ui::new();
    let w = ui.create_window(WindowInfo { title: "T".into(), ..Default::default() });
    let g = w.add_tab(TabInfo::new("Main")).add_groupbox(GroupboxInfo::left("G"));
    (ui, w, g)
}

fn counter(d: &Dropdown) -> Arc<Mutex<Vec<DropdownValue>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = seen.clone();
    d.on_changed(move |v| s.lock().push(v.clone()));
    seen
}

/// Bottom 21px of the holder: the display box.
fn display_box(h: &Harness, id: NodeId) -> Rect {
    let r = h.rect(id);
    Rect::from_min_max(Pos2::new(r.min.x, r.max.y - 21.0), r.max)
}

fn row_center(h: &Harness, id: NodeId, row: usize, row_h: f32, header: f32) -> Pos2 {
    let b = display_box(h, id);
    Pos2::new(b.center().x, b.max.y + 1.5 + header + row as f32 * row_h + row_h / 2.0)
}

#[test]
fn single_set_value_ignores_unknown_and_clears_on_none() {
    let (_ui, _w, g) = setup();
    let d = g.add_dropdown("D", DropdownInfo::new("D", ["a", "b"]).default_value("a"));
    let seen = counter(&d);
    d.set_value(DropdownValue::Single(Some("nope".into())));
    assert_eq!(d.value(), DropdownValue::Single(Some("a".into())));
    d.set_value(DropdownValue::Single(Some("b".into())));
    assert_eq!(d.value(), DropdownValue::Single(Some("b".into())));
    d.set_value(DropdownValue::Single(None));
    assert_eq!(d.value(), DropdownValue::Single(None));
    // Unknown value still fires (Lua runs RunChanged after every SetValue); disabled never fires.
    assert_eq!(seen.lock().len(), 3);
    d.set_disabled(true);
    d.set_value(DropdownValue::Single(Some("a".into())));
    assert_eq!(d.value(), DropdownValue::Single(Some("a".into())));
    assert_eq!(seen.lock().len(), 3);
}

#[test]
fn multi_bulk_and_prune_semantics() {
    let (_ui, _w, g) = setup();
    let d = g.add_dropdown("M", DropdownInfo::new("M", ["a", "b", "c"]).multi().default_values(["a", "c"]));
    let seen = counter(&d);
    assert_eq!(d.active_values(), vec!["a".to_owned(), "c".to_owned()]);
    // Multi accepts a set; unknown values are dropped.
    d.set_value(DropdownValue::Multi(["b".to_owned(), "zzz".to_owned()].into_iter().collect()));
    assert_eq!(d.active_values(), vec!["b".to_owned()]);
    // allow_null = false: deselect_all keeps the first selectable value.
    d.deselect_all(None);
    assert_eq!(d.active_values(), vec!["a".to_owned()]);
    d.select_all(None);
    assert_eq!(d.active_values().len(), 3);
    let before = seen.lock().len();
    // set_values prunes and fires exactly once; add_values never fires.
    d.set_values(vec!["a".into(), "c".into()]);
    assert_eq!(d.active_values(), vec!["a".to_owned(), "c".to_owned()]);
    assert_eq!(seen.lock().len(), before + 1);
    d.set_values(vec!["a".into(), "c".into(), "d".into()]);
    assert_eq!(seen.lock().len(), before + 1, "nothing pruned, no callback");
    d.add_values(vec!["e".into()]);
    assert_eq!(seen.lock().len(), before + 1);
    assert_eq!(d.value().count(), 2);
}

#[test]
fn inline_list_opens_selects_and_stays_open() {
    let (ui, _w, g) = setup();
    let d = g.add_dropdown("D", DropdownInfo::new("Weapon", ["Knife", "Pistol", "Rifle"]));
    let seen = counter(&d);
    let mut h = Harness::new(ui.clone());
    h.settle(3);
    let b = display_box(&h, d.node_id());
    assert!(b.is_positive(), "box {b:?}");
    h.click(Pos2::new(b.min.x + 30.0, b.center().y));
    h.settle(15);
    let menus = h.last.as_ref().unwrap().layers[Layer::Menus as usize].len();
    assert!(menus > 2, "menu shapes: {menus}");
    // Click the second row.
    h.click(row_center(&h, d.node_id(), 1, 21.0, 0.0));
    h.settle(2);
    assert_eq!(d.value(), DropdownValue::Single(Some("Pistol".into())));
    assert_eq!(seen.lock().len(), 1);
    // Single select keeps the list open.
    let menus = h.last.as_ref().unwrap().layers[Layer::Menus as usize].len();
    assert!(menus > 2, "menu still open: {menus}");
    // Click outside closes it.
    h.click(Pos2::new(1200.0, 780.0));
    h.settle(15);
    assert_eq!(h.last.as_ref().unwrap().layers[Layer::Menus as usize].len(), 0);
}

#[test]
fn multi_select_all_header_and_search_filter() {
    let (ui, _w, g) = setup();
    let d = g.add_dropdown("M", DropdownInfo::new("M", ["Alpha", "Beta", "Gamma"]).multi().searchable());
    let mut h = Harness::new(ui.clone());
    h.settle(3);
    let b = display_box(&h, d.node_id());
    h.click(Pos2::new(b.min.x + 30.0, b.center().y));
    h.settle(15);
    // "Select All" is the left half of the 21px header row.
    let header = Pos2::new(b.min.x + b.width() * 0.25, b.max.y + 1.5 + 10.5);
    h.click(header);
    h.settle(2);
    assert_eq!(d.active_values().len(), 3);
    // Typing into the inline search narrows the rows; "Deselect All" only touches matches.
    h.type_into(d.node_id(), 0, "gam", false);
    h.settle(2);
    let deselect = Pos2::new(b.min.x + b.width() * 0.75, b.max.y + 1.5 + 10.5);
    h.click(deselect);
    h.settle(2);
    assert_eq!(d.active_values(), vec!["Alpha".to_owned(), "Beta".to_owned()]);
}

#[test]
fn expanded_panel_opens_picks_and_releases_the_overlay() {
    let (ui, w, g) = setup();
    let d = g.add_dropdown("D", DropdownInfo::new("D", ["One", "Two"]));
    let t = g.add_toggle("T", ToggleInfo::new("T"));
    let mut h = Harness::new(ui.clone());
    h.settle(3);
    let b = display_box(&h, d.node_id());
    // Expand button sits 30px from the box's right edge.
    h.click(Pos2::new(b.max.x - 30.0, b.center().y));
    h.settle(15);
    assert!(d.is_expanded());
    // Content under the overlay is blocked.
    let tc = h.center(t.node_id());
    h.click(tc);
    h.settle(2);
    assert!(!t.value(), "toggle blocked by the expanded overlay");
    // Pick "Two" from the grid: panel is 70%x72% of the window, first cell at (10,10) below the 34px header.
    let win = ui.node_rect(w.node_id());
    let panel = Rect::from_center_size(win.center(), Vec2::new(win.width() * 0.7, win.height() * 0.72));
    let cols = 2.0;
    let cell_w = (panel.width() - 20.0 - 6.0) / cols;
    let cell = Pos2::new(panel.min.x + 10.0 + cell_w + 6.0 + cell_w / 2.0, panel.min.y + 34.0 + 10.0 + 14.0);
    h.click(cell);
    h.settle(2);
    assert_eq!(d.value(), DropdownValue::Single(Some("Two".into())));
    assert!(!d.is_expanded(), "single select collapses the panel");
    h.settle(20);
    // Overlay released: the toggle is clickable again.
    h.click(tc);
    h.settle(2);
    assert!(t.value(), "content unblocked after collapse");
    // Escape collapses too.
    d.expand();
    h.settle(15);
    assert!(d.is_expanded());
    h.tap(Key::Escape);
    assert!(!d.is_expanded());
}

#[test]
fn priority_set_value_orders_and_drag_reorders() {
    let (ui, _w, g) = setup();
    let p = g.add_priority_dropdown("P", PriorityDropdownInfo::new("P", ["a", "b", "c"]));
    let seen = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
    let s = seen.clone();
    p.on_changed(move |v| s.lock().push(v.to_vec()));
    p.set_value(vec!["c".into(), "zzz".into(), "a".into()]);
    assert_eq!(p.value(), vec!["c".to_owned(), "a".to_owned(), "b".to_owned()]);
    assert_eq!(seen.lock().len(), 1);
    p.set_values(vec!["b".into(), "a".into(), "d".into()]);
    assert_eq!(p.value(), vec!["a".to_owned(), "b".to_owned(), "d".to_owned()], "relative order kept, unknown dropped, new appended");
    assert_eq!(seen.lock().len(), 2);

    let mut h = Harness::new(ui.clone());
    h.settle(3);
    let b = display_box(&h, p.node_id());
    h.click(Pos2::new(b.min.x + 30.0, b.center().y));
    h.settle(15);
    // Drag row 0 ("a") down past row 1: rows are 24px.
    let r0 = row_center(&h, p.node_id(), 0, 24.0, 0.0);
    h.press(r0, MouseButton::Left);
    h.frame(1.0 / 60.0);
    for k in 1..=8 {
        h.move_to(Pos2::new(r0.x, r0.y + 24.0 * 1.4 * k as f32 / 8.0));
        h.frame(1.0 / 60.0);
    }
    assert_eq!(p.value(), vec!["b".to_owned(), "a".to_owned(), "d".to_owned()], "reordered live while dragging");
    h.release(MouseButton::Left);
    h.frame(1.0 / 60.0);
    h.settle(2);
    assert_eq!(seen.lock().len(), 3, "listener fires once after the drag release");
    assert_eq!(p.value(), vec!["b".to_owned(), "a".to_owned(), "d".to_owned()]);
}

#[test]
fn multi_drag_select_toggles_a_range_and_fires_once() {
    let (ui, _w, g) = setup();
    let d = g.add_dropdown("M", DropdownInfo { drag_select: true, ..DropdownInfo::new("M", ["a", "b", "c", "d"]).multi() });
    let seen = counter(&d);
    let mut h = Harness::new(ui.clone());
    h.settle(3);
    let b = display_box(&h, d.node_id());
    h.click(Pos2::new(b.min.x + 30.0, b.center().y));
    h.settle(15);
    // Press on row 0 (below the 21px Select-All header), sweep to row 2, release.
    let r0 = row_center(&h, d.node_id(), 0, 21.0, 21.0);
    h.press(r0, MouseButton::Left);
    h.frame(1.0 / 60.0);
    assert_eq!(d.active_values(), vec!["a".to_owned()], "press toggles the start row");
    h.move_to(row_center(&h, d.node_id(), 2, 21.0, 21.0));
    h.frame(1.0 / 60.0);
    assert_eq!(d.active_values(), vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
    // Sweeping back shrinks the range (rows leaving revert to their initial state).
    h.move_to(row_center(&h, d.node_id(), 1, 21.0, 21.0));
    h.frame(1.0 / 60.0);
    assert_eq!(d.active_values(), vec!["a".to_owned(), "b".to_owned()]);
    assert!(seen.lock().is_empty(), "no callback until release");
    h.release(MouseButton::Left);
    h.frame(1.0 / 60.0);
    h.settle(2);
    assert_eq!(seen.lock().len(), 1);
}

#[test]
fn priority_expanded_panel_drags_and_collapses() {
    let (ui, w, g) = setup();
    let p = g.add_priority_dropdown("P", PriorityDropdownInfo::new("P", ["a", "b", "c"]));
    let mut h = Harness::new(ui.clone());
    h.settle(3);
    p.expand();
    h.settle(15);
    assert!(p.is_expanded());
    // Panel is 60%x68% of the window; searchable -> list starts at 72 (+10 padding), rows pitch 34.
    let win = ui.node_rect(w.node_id());
    let panel = Rect::from_center_size(win.center(), Vec2::new(win.width() * 0.6, win.height() * 0.68));
    let row = |i: f32| Pos2::new(panel.min.x + 10.0 + 80.0, panel.min.y + 72.0 + i * 34.0 + 15.0);
    h.press(row(2.0), MouseButton::Left);
    h.frame(1.0 / 60.0);
    for k in 1..=10 {
        h.move_to(Pos2::new(row(0.0).x, row(2.0).y - 34.0 * 2.2 * k as f32 / 10.0));
        h.frame(1.0 / 60.0);
    }
    h.release(MouseButton::Left);
    h.frame(1.0 / 60.0);
    assert_eq!(p.value(), vec!["c".to_owned(), "a".to_owned(), "b".to_owned()]);
    // Click the dimmed overlay outside the panel -> collapse and release the overlay.
    h.click(Pos2::new(win.min.x + 4.0, win.max.y - 4.0));
    assert!(!p.is_expanded());
    h.settle(20);
    let b = display_box(&h, p.node_id());
    h.click(Pos2::new(b.min.x + 30.0, b.center().y));
    h.settle(15);
    assert!(h.last.as_ref().unwrap().layers[Layer::Menus as usize].len() > 2, "inline list opens again after collapse");
}
