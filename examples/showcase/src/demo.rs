//! Builds the showcase UI (grows as elements land).

use obsidian_ultra_core::*;

pub fn build(ui: &Ui) {
    let window = ui.create_window(WindowInfo {
        title: "Obsidian Ultra".into(),
        footer: Footer::Segments(vec![
            FooterSegment { text: "v0.1.0".into(), copyable: false, copy_text: None },
            FooterSegment { text: "discord.gg/example".into(), copyable: true, copy_text: None },
        ]),
        icon: Some("gem".into()),
        ..Default::default()
    });

    let main = window.add_tab(TabInfo::new("Main").icon("house").description("Every element in one place"));
    let combat = main.add_groupbox(GroupboxInfo::left("Combat").icon("crosshair").description("Aim and targeting"));
    combat.add_toggle("Aimbot", ToggleInfo::new("Enable aimbot").default_value(true).tooltip("Locks onto the nearest target"));
    combat.add_checkbox("Visible", ToggleInfo::new("Visible check"));
    combat.add_slider("Fov", SliderInfo::new("FOV", 0.0, 180.0, 90.0).suffix("°"));
    combat.add_slider("Smooth", SliderInfo { compact: true, ..SliderInfo::new("Smoothing", 0.0, 1.0, 0.25).rounding(2) });
    combat.add_input("Name", InputInfo { placeholder: "player name".into(), ..InputInfo::new("Target name") });
    combat.add_button(ButtonInfo::new("Reset", || {}));
    let b = combat.add_button(ButtonInfo { double_click: true, risky: true, ..ButtonInfo::new("Clear config", || {}) });
    b.add_sub_button(ButtonInfo::new("Sub", || {}));
    combat.add_divider(DividerInfo::text("Misc"));
    combat.add_label(LabelInfo::new("A wrapped label showing how long text flows across several lines inside a groupbox.").wrap());

    let visuals = main.add_groupbox(GroupboxInfo::right("Visuals").icon("eye"));
    visuals.add_toggle("Esp", ToggleInfo::new("ESP").tooltip("Draw boxes"));
    visuals.add_toggle("Chams", ToggleInfo { risky: true, ..ToggleInfo::new("Chams") });
    visuals.add_toggle("Disabled", ToggleInfo { disabled: true, disabled_tooltip: Some("Coming soon".into()), ..ToggleInfo::new("Disabled toggle") });
    visuals.add_slider("Thick", SliderInfo::new("Box thickness", 1.0, 5.0, 2.0));
    let esp_color = visuals.add_toggle("EspColor", ToggleInfo::new("ESP color"));
    esp_color.add_color_picker("EspColorPicker", ColorPickerInfo::new(Color32::from_rgb(125, 85, 255)).with_alpha(0.0).title("ESP color"));
    let kp_host = visuals.add_toggle("Trigger", ToggleInfo::new("Triggerbot"));
    kp_host.add_key_picker("TriggerKey", KeyPickerInfo::key("Triggerbot", Key::X).mode(KeyMode::Hold));
    let menu_key = visuals.add_label(LabelInfo::new("Menu keybind")).add_key_picker("MenuKey", KeyPickerInfo::key("Menu", Key::RightControl).mode(KeyMode::Toggle));
    ui.set_toggle_keybind_picker(Some(&menu_key));
    visuals.add_dropdown("Bone", DropdownInfo::new("Target bone", ["Head", "Neck", "Torso", "Pelvis"]).default_value("Head"));
    visuals.add_dropdown("Parts", DropdownInfo::new("Visible parts", ["Box", "Name", "Health", "Distance", "Weapon", "Skeleton", "Chams", "Tracer", "Snapline"]).multi().searchable().default_values(["Box", "Name"]));
    visuals.add_priority_dropdown("Priority", PriorityDropdownInfo::new("Target priority", ["Distance", "Health", "Threat", "FOV"]));

    let media = main.add_groupbox(GroupboxInfo::right("Media").icon("image"));
    media.add_image("Logo", ImageInfo { image: "gem".into(), height: 60.0, color: Color32::from_rgb(125, 85, 255), ..Default::default() });
    media.add_profile_card("Profile", ProfileCardInfo { title: "sinnafuls".into(), description: Description::Lines(vec![DescriptionLine::Text("Premium".into()), DescriptionLine::Divider, DescriptionLine::Text("Since 2026".into())]), height: Some(64.0), ..ProfileCardInfo::new("sinnafuls") });
    media.add_custom("Wave", CustomInfo::new(40.0, |c| {
        let n = 40;
        let w = c.rect.width();
        let pts: Vec<Pos2> = (0..=n).map(|i| {
            let t = i as f32 / n as f32;
            Pos2::new(c.rect.min.x + t * w, c.rect.center().y + ((t * 12.0 + c.time as f32 * 3.0).sin() * c.rect.height() * 0.4))
        }).collect();
        c.shapes.push(epaint::Shape::line(pts, epaint::Stroke::new(2.0 * c.scale, c.scheme.accent)));
    }));
    let dep = combat.add_dependency_box();
    dep.add_label(LabelInfo::new("Shown only while aimbot is on"));
    dep.add_slider("AimSmooth", SliderInfo::new("Aim smoothing", 0.0, 10.0, 5.0));
    dep.set_dependencies(vec![Dependency::Toggle(ui.toggle_by("Aimbot").unwrap(), true)]);
    let settings = window.add_tab(TabInfo::new("Settings").icon("settings").description("Library settings"));
    settings.update_warning_box(WarningBoxInfo { visible: Some(true), title: Some("HEADS UP".into()), text: Some("Everything on this tab changes the library live: DPI, corner radius, theme colors, glow and snapping.".into()), ..Default::default() });
    let g = settings.add_groupbox(GroupboxInfo::left("Menu").icon("sliders-horizontal"));
    g.add_label(LabelInfo::new("Use RightControl to toggle the menu."));
    let ui2 = ui.clone();
    g.add_slider("Dpi", SliderInfo { callback: Some(Box::new(move |v| ui2.set_dpi_scale(v as f32))), ..SliderInfo::new("DPI scale", 50.0, 200.0, 100.0).suffix("%") });
    let ui2 = ui.clone();
    g.add_slider("Radius", SliderInfo { callback: Some(Box::new(move |v| ui2.set_corner_radius(v as f32))), ..SliderInfo::new("Corner radius", 0.0, 20.0, 4.0) });
    let w2 = window.clone();
    g.add_toggle("Glow", ToggleInfo { callback: Some(Box::new(move |on| w2.set_glow(on, GlowOptions::default()))), ..ToggleInfo::new("Window glow") });
    let w2 = window.clone();
    g.add_toggle("Snap", ToggleInfo { callback: Some(Box::new(move |on| w2.set_snapping(on, None, None))), ..ToggleInfo::new("Edge snapping while dragging") });
    let ui2 = ui.clone();
    g.add_toggle("Anim", ToggleInfo { callback: Some(Box::new(move |on| ui2.settings_mut(|s| s.animations = if on { Animations::ALL } else { Animations::default() }))), ..ToggleInfo::new("All animations") });
    let ui2 = ui.clone();
    g.add_toggle("Cursor", ToggleInfo { default: true, callback: Some(Box::new(move |on| ui2.settings_mut(|s| s.show_custom_cursor = on))), ..ToggleInfo::new("Custom cursor") });
    let ui2 = ui.clone();
    g.add_dropdown("Notify", DropdownInfo { callback: Some(Box::new(move |v| { if let DropdownValue::Single(Some(s)) = v { ui2.set_notify_side(if s == "Left" { Side::Left } else { Side::Right }); } })), ..DropdownInfo::new("Notification side", ["Right", "Left"]).default_value("Right") });
    let ui2 = ui.clone();
    g.add_button(ButtonInfo::new("Unsupported executor screen", move || {
        // Demonstrates the matching logic; a real script would call this before creating its window.
        let probe = Ui::new();
        let shown = probe.create_unsupported_screen(UnsupportedInfo { executor: "Solara".into(), supported: Some(vec!["Synapse".into(), "Wave".into()]), ..Default::default() }).is_some();
        ui2.notify(NotifyInfo::new("Unsupported screen", if shown { "Solara is not in the supported list, screen would show" } else { "supported" }).kind(NotifyType::Info));
    }));

    let theme = settings.add_groupbox(GroupboxInfo::left("Theme").icon("palette").description("Bound to Ui::update_scheme"));
    macro_rules! theme_color {
        ($idx:literal, $label:literal, $field:ident, $default:expr) => {{
            let ui2 = ui.clone();
            let l = theme.add_label(LabelInfo::new($label));
            l.add_color_picker($idx, ColorPickerInfo { callback: Some(Box::new(move |c| ui2.update_scheme(|s| s.$field = c.color))), ..ColorPickerInfo::new($default) });
        }};
    }
    theme_color!("AccentColor", "Accent", accent, Color32::from_rgb(125, 85, 255));
    theme_color!("BackgroundColor", "Background", background, Color32::from_rgb(15, 15, 15));
    theme_color!("MainColor", "Main", main, Color32::from_rgb(25, 25, 25));
    theme_color!("OutlineColor", "Outline", outline, Color32::from_rgb(40, 40, 40));
    theme_color!("FontColor", "Font", font_color, Color32::WHITE);
    let tb = settings.add_tabbox(TabboxInfo::new(Side::Right));
    let ta = tb.add_tab(Some("Theme"), Some("palette".into()));
    ta.add_slider("Radius", SliderInfo::new("Corner radius", 0.0, 20.0, 4.0));
    let tb2 = tb.add_tab(Some("Misc"), None);
    tb2.add_toggle("Misc1", ToggleInfo::new("Misc toggle"));

    let players = window.add_tab(TabInfo::new("Players").icon("users").description("Sub tabs demo"));
    let list = players.add_sub_tab(SubTabInfo { icon: Some("list".into()), ..SubTabInfo::new("List") });
    list.add_groupbox(GroupboxInfo::left("Online")).add_label(LabelInfo::new("Nobody here yet."));
    let friends = players.add_sub_tab(SubTabInfo::new("Friends"));
    friends.add_groupbox(GroupboxInfo::left("Friends")).add_toggle("Friendly", ToggleInfo::new("Ignore friends"));

    let key = window.add_key_tab(KeyTabInfo { name: "Key".into(), description: Some("Key system".into()), ..Default::default() });
    key.add_label(LabelInfo::new("Enter your key below"));
    key.add_key_box(|k| eprintln!("key submitted: {k}"));
    // Overlays.
    let overlays = window.add_tab(TabInfo::new("Overlays").icon("bell").description("Notifications, dialogs, floats"));
    let notif = overlays.add_groupbox(GroupboxInfo::left("Notifications").icon("bell"));
    for (kind, name) in [(NotifyType::Info, "Info"), (NotifyType::Success, "Success"), (NotifyType::Warning, "Warning"), (NotifyType::Error, "Error")] {
        let ui2 = ui.clone();
        notif.add_button(ButtonInfo::new(format!("Notify {name}"), move || {
            ui2.notify(NotifyInfo::new(name, format!("This is a {} notification", name.to_lowercase())).kind(kind).time(4.0));
        }));
    }
    let ui2 = ui.clone();
    notif.add_button(ButtonInfo::new("Progress notification", move || {
        let n = ui2.notify(NotifyInfo { steps: Some(4), persist: true, ..NotifyInfo::new("Loading", "Step 2 of 4") });
        n.set_step(2);
    }));
    let ui2 = ui.clone();
    notif.add_button(ButtonInfo::new("Toggle history panel", move || ui2.toggle_notification_history()));

    let dialogs = overlays.add_groupbox(GroupboxInfo::right("Dialogs").icon("message-square"));
    let w2 = window.clone();
    dialogs.add_button(ButtonInfo::new("Open dialog", move || {
        let d = w2.add_dialog(
            "Confirm",
            DialogInfo {
                title: "Reset settings?".into(),
                description: "This will restore every option to its default value.".into(),
                icon: Some("triangle-alert".into()),
                ..Default::default()
            },
        );
        d.add_footer_button("cancel", FooterButtonInfo { title: Some("Cancel".into()), variant: ButtonVariant::Secondary, order: 0, ..Default::default() });
        d.add_footer_button("reset", FooterButtonInfo { title: Some("Reset".into()), variant: ButtonVariant::Destructive, order: 1, wait_time: Some(2.0), callback: Some(Box::new(|_d| eprintln!("reset!"))), ..Default::default() });
    }));
    let floats = overlays.add_groupbox(GroupboxInfo::right("Floats").icon("move"));
    let ui2 = ui.clone();
    floats.add_button(ButtonInfo::new("Add draggable label", move || {
        ui2.add_draggable_label(DraggableLabelInfo { icon: Some("tag".into()), ..DraggableLabelInfo::new("Draggable label") });
    }));
    let ui2 = ui.clone();
    floats.add_button(ButtonInfo::new("Add draggable button", move || {
        ui2.add_draggable_button(DraggableButtonInfo::new("Click me", || eprintln!("draggable button clicked")));
    }));
    let ui2 = ui.clone();
    floats.add_button(ButtonInfo::new("Show watermark", move || {
        let wm = ui2.watermark();
        wm.set_segments(vec![WatermarkSegment::text("Obsidian Ultra").accent().icon("gem"), WatermarkSegment::getter(|| format!("{} fps", 144)), WatermarkSegment::text("v0.1.0")]);
        wm.set_visible(true);
    }));
    let ui2 = ui.clone();
    floats.add_button(ButtonInfo::new("Loading screen", move || {
        let l = ui2.create_loading(LoadingInfo { title: "Obsidian Ultra".into(), current_step: 3, total_steps: 5, ..Default::default() });
        l.set_message("Loading modules...");
        l.set_description("Please wait");
        let l2 = l.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(3));
            l2.destroy();
        });
    }));
    main.show();
    if std::env::var_os("OBSIDIAN_DEMO_SETTINGS").is_some() {
        settings.show();
    }
    if std::env::var_os("OBSIDIAN_DEMO_EXPAND").is_some() {
        ui.option("Parts").and_then(|o| match o { OptionHandle::Dropdown(d) => Some(d), _ => None }).unwrap().expand();
    }
    if std::env::var_os("OBSIDIAN_DEMO_PRIORITY").is_some() {
        ui.option("Priority").and_then(|o| match o { OptionHandle::PriorityDropdown(d) => Some(d), _ => None }).unwrap().expand();
    }
    if std::env::var_os("OBSIDIAN_DEMO_OVERLAYS").is_some() {
        overlays.show();
        ui.notify(NotifyInfo::new("Success", "Config loaded").kind(NotifyType::Success).time(30.0));
        ui.notify(NotifyInfo { big_icon: Some("triangle-alert".into()), ..NotifyInfo::new("Warning", "Something needs attention here, with a longer description line").kind(NotifyType::Warning).time(30.0) });
        let n = ui.notify(NotifyInfo { steps: Some(4), persist: true, ..NotifyInfo::new("Loading", "Step 2 of 4") });
        n.set_step(2);
        ui.add_draggable_label(DraggableLabelInfo { icon: Some("tag".into()), ..DraggableLabelInfo::new("Draggable label") });
        ui.add_draggable_button(DraggableButtonInfo::new("Click me", || {}));
        let wm = ui.watermark();
        wm.set_segments(vec![WatermarkSegment::text("Obsidian Ultra").accent().icon("gem"), WatermarkSegment::text("144 fps"), WatermarkSegment::text("v0.1.0")]);
        wm.set_visible(true);
        if std::env::var_os("OBSIDIAN_DEMO_DIALOG").is_some() {
            let d = window.add_dialog("Confirm", DialogInfo { title: "Reset settings?".into(), description: "This will restore every option to its default value.".into(), icon: Some("triangle-alert".into()), ..Default::default() });
            d.add_footer_button("cancel", FooterButtonInfo { title: Some("Cancel".into()), variant: ButtonVariant::Secondary, order: 0, ..Default::default() });
            d.add_footer_button("reset", FooterButtonInfo { title: Some("Reset".into()), variant: ButtonVariant::Destructive, order: 1, wait_time: Some(2.0), ..Default::default() });
        }
        if std::env::var_os("OBSIDIAN_DEMO_HISTORY").is_some() {
            ui.set_notification_history_visible(true);
        }
        if std::env::var_os("OBSIDIAN_DEMO_LOADING").is_some() {
            let l = ui.create_loading(LoadingInfo { title: "Obsidian Ultra".into(), current_step: 3, total_steps: 5, ..Default::default() });
            l.set_message("Loading modules...");
            l.set_description("Please wait");
        }
    }
}
