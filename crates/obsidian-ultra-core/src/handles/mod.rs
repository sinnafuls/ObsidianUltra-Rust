//! Public handles: cheap, `Send + Sync` references to retained nodes.

pub mod info;

use std::collections::HashMap;

use epaint::{Color32, Pos2, Rect, Shape, Vec2};

pub use info::*;

use crate::color::Rgba;
use crate::node::*;
use crate::scheme::Scheme;
use crate::types::*;
use crate::ui::{Model, Pending, Ui};

macro_rules! handle {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone)]
        pub struct $name {
            pub(crate) ui: Ui,
            pub(crate) id: NodeId,
        }

        impl $name {
            /// Registry key (`""` when unregistered).
            pub fn id(&self) -> String {
                self.ui.with(|m| m.nodes.get(self.id).map(|n| n.idx.clone()).unwrap_or_default())
            }

            pub fn node_id(&self) -> NodeId {
                self.id
            }

            pub fn is_destroyed(&self) -> bool {
                self.ui.with(|m| !m.alive(self.id))
            }

            pub fn set_visible(&self, visible: bool) {
                self.ui.mutate(|m| {
                    if let Some(n) = m.nodes.get_mut(self.id) {
                        n.visible = visible;
                    }
                    crate::window::after_visibility_change(m, self.id, visible);
                });
            }

            pub fn destroy(&self) {
                self.ui.mutate(|m| crate::window::destroy_node(m, self.id));
            }

            pub fn ui(&self) -> &Ui {
                &self.ui
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }
        impl Eq for $name {}
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({:?})", stringify!($name), self.id)
            }
        }
    };
}

handle!(/// The main window.
    Window);
handle!(/// A sidebar tab.
    Tab);
handle!(/// A sub-tab nested under a tab.
    SubTab);
handle!(/// A tabbox (inline tab strip) inside a column or groupbox.
    Tabbox);
handle!(/// One tab of a tabbox; a [`Container`].
    TabboxTab);
handle!(/// A groupbox; a [`Container`].
    Groupbox);
handle!(/// Inline dependency box; a [`Container`] shown only when its dependencies hold.
    DependencyBox);
handle!(/// Sibling dependency groupbox; a [`Container`].
    DependencyGroupbox);
handle!(/// A modal dialog; a [`Container`].
    Dialog);
handle!(/// A key-system tab; a [`Container`].
    KeyTab);
handle!(Divider);
handle!(Label);
handle!(/// A button (or half-width sub-button).
    Button);
handle!(Toggle);
handle!(Input);
handle!(Slider);
handle!(Dropdown);
handle!(PriorityDropdown);
handle!(Image);
handle!(ProfileCard);
handle!(/// Custom-drawn element.
    Custom);
handle!(KeyPicker);
handle!(ColorPicker);
handle!(Notification);
handle!(/// The loading screen.
    Loading);
handle!(/// The loading screen's sidebar; a [`Container`].
    LoadingSidebar);
handle!(Watermark);
handle!(DraggableLabel);
handle!(DraggableButton);
handle!(/// A draggable floating menu; a [`Container`].
    DraggableMenu);
handle!(DraggableImageButton);
handle!(MinimizedLabel);

/// Sub-buttons are buttons.
pub type SubButton = Button;

/// Context passed to [`Custom`] draw closures every frame.
pub struct CustomCtx<'a> {
    /// Screen rect of the element (already scaled and clipped).
    pub rect: Rect,
    pub input: &'a crate::input::Input,
    pub scheme: &'a Scheme,
    /// DPI scale factor.
    pub scale: f32,
    pub time: f64,
    /// Push shapes here (screen coordinates).
    pub shapes: &'a mut Vec<Shape>,
    /// The pointer is over this element and not blocked by an overlay.
    pub hovered: bool,
}

/// Any option registered in `Options`.
#[derive(Clone, Debug)]
pub enum OptionHandle {
    Input(Input),
    Slider(Slider),
    Dropdown(Dropdown),
    PriorityDropdown(PriorityDropdown),
    KeyPicker(KeyPicker),
    ColorPicker(ColorPicker),
    ProfileCard(ProfileCard),
    Image(Image),
    Custom(Custom),
}

impl OptionHandle {
    pub(crate) fn from_node(ui: &Ui, m: &Model, id: NodeId) -> Option<Self> {
        let n = m.nodes.get(id)?;
        let ui = ui.clone();
        Some(match &n.kind {
            Kind::Input(_) => OptionHandle::Input(Input { ui, id }),
            Kind::Slider(_) => OptionHandle::Slider(Slider { ui, id }),
            Kind::Dropdown(_) => OptionHandle::Dropdown(Dropdown { ui, id }),
            Kind::Priority(_) => OptionHandle::PriorityDropdown(PriorityDropdown { ui, id }),
            Kind::KeyPicker(_) => OptionHandle::KeyPicker(KeyPicker { ui, id }),
            Kind::ColorPicker(_) => OptionHandle::ColorPicker(ColorPicker { ui, id }),
            Kind::ProfileCard(_) => OptionHandle::ProfileCard(ProfileCard { ui, id }),
            Kind::Image(_) => OptionHandle::Image(Image { ui, id }),
            Kind::Custom(_) => OptionHandle::Custom(Custom { ui, id }),
            _ => return None,
        })
    }

    pub fn value(&self) -> Value {
        match self {
            OptionHandle::Input(i) => Value::Text(i.value()),
            OptionHandle::Slider(s) => Value::Number(s.value()),
            OptionHandle::Dropdown(d) => Value::Dropdown(d.value()),
            OptionHandle::PriorityDropdown(p) => Value::Priority(p.value()),
            OptionHandle::KeyPicker(k) => Value::Key(k.value()),
            OptionHandle::ColorPicker(c) => Value::Color(c.value()),
            _ => Value::None,
        }
    }
}

// ===== Container trait ======================================================================

/// Element constructors available on every container (`BaseGroupbox` mixin).
pub trait Container {
    #[doc(hidden)]
    fn __ui(&self) -> &Ui;
    #[doc(hidden)]
    fn __id(&self) -> NodeId;

    fn add_divider(&self, info: DividerInfo) -> Divider {
        let id = self.__ui().mutate(|m| crate::elements::basic::create_divider(m, self.__id(), info));
        Divider { ui: self.__ui().clone(), id }
    }

    fn add_label(&self, info: LabelInfo) -> Label {
        self.add_label_idx("", info)
    }

    fn add_label_idx(&self, idx: &str, info: LabelInfo) -> Label {
        let id = self.__ui().mutate(|m| crate::elements::basic::create_label(m, self.__id(), idx, info));
        Label { ui: self.__ui().clone(), id }
    }

    fn add_button(&self, info: ButtonInfo) -> Button {
        self.add_button_idx("", info)
    }

    fn add_button_idx(&self, idx: &str, info: ButtonInfo) -> Button {
        let id = self.__ui().mutate(|m| crate::elements::basic::create_button(m, self.__id(), idx, info, false));
        Button { ui: self.__ui().clone(), id }
    }

    fn add_checkbox(&self, idx: &str, info: ToggleInfo) -> Toggle {
        let id = self.__ui().mutate(|m| crate::elements::toggle::create(m, self.__id(), idx, info, ToggleVariant::Checkbox));
        Toggle { ui: self.__ui().clone(), id }
    }

    fn add_toggle(&self, idx: &str, info: ToggleInfo) -> Toggle {
        let id = self.__ui().mutate(|m| {
            let variant = if m.settings.force_checkbox { ToggleVariant::Checkbox } else { ToggleVariant::Switch };
            crate::elements::toggle::create(m, self.__id(), idx, info, variant)
        });
        Toggle { ui: self.__ui().clone(), id }
    }

    fn add_input(&self, idx: &str, info: InputInfo) -> Input {
        let id = self.__ui().mutate(|m| crate::elements::input::create(m, self.__id(), idx, info));
        Input { ui: self.__ui().clone(), id }
    }

    fn add_slider(&self, idx: &str, info: SliderInfo) -> Slider {
        let id = self.__ui().mutate(|m| crate::elements::slider::create(m, self.__id(), idx, info));
        Slider { ui: self.__ui().clone(), id }
    }

    fn add_dropdown(&self, idx: &str, info: DropdownInfo) -> Dropdown {
        let id = self.__ui().mutate(|m| crate::elements::dropdown::create(m, self.__id(), idx, info));
        Dropdown { ui: self.__ui().clone(), id }
    }

    fn add_priority_dropdown(&self, idx: &str, info: PriorityDropdownInfo) -> PriorityDropdown {
        let id = self.__ui().mutate(|m| crate::elements::priority::create(m, self.__id(), idx, info));
        PriorityDropdown { ui: self.__ui().clone(), id }
    }

    fn add_image(&self, idx: &str, info: ImageInfo) -> Image {
        let id = self.__ui().mutate(|m| crate::elements::media::create_image(m, self.__id(), idx, info));
        Image { ui: self.__ui().clone(), id }
    }

    fn add_profile_card(&self, idx: &str, info: ProfileCardInfo) -> ProfileCard {
        let id = self.__ui().mutate(|m| crate::elements::media::create_profile(m, self.__id(), idx, info, true));
        ProfileCard { ui: self.__ui().clone(), id }
    }

    fn add_custom(&self, idx: &str, info: CustomInfo) -> Custom {
        let id = self.__ui().mutate(|m| crate::elements::media::create_custom(m, self.__id(), idx, info));
        Custom { ui: self.__ui().clone(), id }
    }

    fn add_dependency_box(&self) -> DependencyBox {
        let id = self.__ui().mutate(|m| crate::elements::dependency::create(m, self.__id(), false));
        DependencyBox { ui: self.__ui().clone(), id }
    }
}

macro_rules! container {
    ($($t:ident),*) => {$(
        impl Container for $t {
            fn __ui(&self) -> &Ui { &self.ui }
            fn __id(&self) -> NodeId { self.id }
        }
    )*};
}
container!(Groupbox, TabboxTab, DependencyBox, DependencyGroupbox, Dialog, KeyTab, LoadingSidebar, DraggableMenu);

// ===== Window ===============================================================================

impl Window {
    pub fn set_title(&self, title: &str) {
        self.ui.with(|m| {
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                w.title = title.to_owned();
            }
        });
    }

    pub fn set_background_image(&self, image: Option<IconRef>) {
        self.ui.with(|m| {
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                w.background_image = image;
            }
        });
    }

    pub fn set_glow(&self, enabled: bool, options: GlowOptions) {
        self.ui.with(|m| {
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                w.glow.enabled = enabled;
                if let Some(t) = options.transparency {
                    w.glow.transparency = t.clamp(0.0, 1.0);
                }
                if let Some(r) = options.radius {
                    w.glow.radius = r.max(0.0);
                }
                if enabled {
                    w.glow.color = options.color;
                }
            }
        });
    }

    pub fn size_position(&self) -> (Vec2, Pos2) {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Window(w) => (w.size, w.pos),
            _ => (Vec2::ZERO, Pos2::ZERO),
        })
    }

    pub fn set_size_position(&self, size: Option<Vec2>, position: Option<Pos2>) {
        self.ui.with(|m| {
            let screen = m.screen;
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                if let Some(s) = size {
                    let max_x = (screen.width() - 64.0).max(w.min_size.x);
                    let max_y = (screen.height() - 64.0).max(w.min_size.y);
                    w.size = Vec2::new(s.x.clamp(w.min_size.x, max_x), s.y.clamp(w.min_size.y, max_y));
                }
                if let Some(p) = position {
                    w.pos = p;
                }
            }
        });
    }

    pub fn set_footer(&self, footer: impl Into<Footer>) {
        let footer = footer.into();
        self.ui.with(|m| {
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                w.footer = footer;
            }
        });
    }

    pub fn set_always_on_top(&self, enabled: bool) {
        self.ui.with(|m| {
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                w.always_on_top = enabled;
            }
        });
    }

    pub fn always_on_top(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Window(w) => w.always_on_top,
            _ => false,
        })
    }

    pub fn set_snapping(&self, enabled: bool, distance: Option<f32>, margin: Option<f32>) {
        self.ui.with(|m| {
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                w.snapping = enabled;
                if let Some(d) = distance {
                    w.snap_distance = d.max(0.0);
                }
                if let Some(mg) = margin {
                    w.snap_margin = mg.max(0.0);
                }
            }
        });
    }

    pub fn set_corner_radius(&self, radius: f32) {
        self.ui.set_corner_radius(radius);
    }

    pub fn set_animations(&self, animations: Option<Animations>, transition_time: Option<f32>, swipe_offset: Option<f32>, swipe_from: Option<SwipeFrom>) {
        self.ui.with(|m| {
            if let Some(a) = animations {
                m.settings.animations = a;
            }
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                if let Some(t) = transition_time {
                    w.tab_transition_time = t.max(0.0);
                }
                if let Some(o) = swipe_offset {
                    w.tab_swipe_offset = o.max(1.0);
                }
                if let Some(f) = swipe_from {
                    w.tab_swipe_from = f;
                }
            }
        });
    }

    pub fn is_sidebar_compacted(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Window(w) => w.compact,
            _ => false,
        })
    }

    pub fn set_compact(&self, compact: bool) {
        self.ui.with(|m| {
            let target = match &m.nodes[self.id].kind {
                Kind::Window(w) => {
                    if compact { w.info.sidebar_compact_width } else { w.last_expanded_width }
                }
                _ => return,
            };
            crate::window::set_sidebar_width(m, self.id, target);
        });
    }

    pub fn sidebar_width(&self) -> f32 {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Window(w) => w.sidebar_width,
            _ => 0.0,
        })
    }

    pub fn set_sidebar_width(&self, width: f32) {
        self.ui.with(|m| crate::window::set_sidebar_width(m, self.id, width));
    }

    pub fn is_minimized(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Window(w) => w.minimized,
            _ => false,
        })
    }

    pub fn set_minimized(&self, value: Option<bool>) {
        self.ui.with(|m| crate::window::set_minimized(m, self.id, value));
    }

    pub fn toggle_minimized(&self) {
        self.set_minimized(None);
    }

    pub fn set_minimized_subtitle(&self, text: Option<&str>) {
        self.ui.with(|m| {
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                let t = text.unwrap_or("");
                w.mini_subtitle_explicit = !t.is_empty();
                w.mini_subtitle = t.to_owned();
            }
        });
    }

    pub fn add_minimized_label(&self, text: &str) -> MinimizedLabel {
        let id = self.ui.with(|m| {
            let id = m.insert(Kind::MinimizedLabel(MinimizedLabelData { text: text.to_owned() }), Some(self.id));
            if let Kind::Window(w) = &mut m.nodes[self.id].kind {
                w.mini_labels.push(id);
            }
            id
        });
        MinimizedLabel { ui: self.ui.clone(), id }
    }

    pub fn clear_minimized_labels(&self) {
        self.ui.with(|m| {
            let labels = match &m.nodes[self.id].kind {
                Kind::Window(w) => w.mini_labels.clone(),
                _ => Vec::new(),
            };
            for l in labels.into_iter().rev() {
                m.destroy_subtree(l);
            }
        });
    }

    pub fn add_tab(&self, info: TabInfo) -> Tab {
        let id = self.ui.mutate(|m| crate::window::tabs::create_tab(m, self.id, info));
        Tab { ui: self.ui.clone(), id }
    }

    pub fn add_key_tab(&self, info: KeyTabInfo) -> KeyTab {
        let id = self.ui.mutate(|m| crate::window::tabs::create_key_tab(m, self.id, info));
        KeyTab { ui: self.ui.clone(), id }
    }

    pub fn add_dialog(&self, idx: &str, info: DialogInfo) -> Dialog {
        let id = self.ui.mutate(|m| crate::overlays::dialog::create(m, self.id, idx, info));
        Dialog { ui: self.ui.clone(), id }
    }

    /// Show/hide the whole UI (same as [`Ui::toggle`]).
    pub fn toggle(&self, value: Option<bool>) {
        self.ui.toggle(value);
    }
}

impl MinimizedLabel {
    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Some(Kind::MinimizedLabel(l)) = m.nodes.get_mut(self.id).map(|n| &mut n.kind) {
                l.text = text.to_owned();
            }
        });
    }
}

// ===== Tab / SubTab =========================================================================

macro_rules! box_adders {
    ($t:ident) => {
        impl $t {
            pub fn add_groupbox(&self, info: GroupboxInfo) -> Groupbox {
                let id = self.ui.mutate(|m| crate::window::boxes::create_groupbox(m, self.id, info));
                Groupbox { ui: self.ui.clone(), id }
            }

            pub fn add_tabbox(&self, info: TabboxInfo) -> Tabbox {
                let id = self.ui.mutate(|m| crate::window::boxes::create_tabbox(m, self.id, info));
                Tabbox { ui: self.ui.clone(), id }
            }

            pub fn show(&self) {
                self.ui.mutate(|m| crate::window::tabs::show(m, self.id));
            }

            pub fn hide(&self) {
                self.ui.mutate(|m| crate::window::tabs::hide(m, self.id));
            }
        }
    };
}
box_adders!(Tab);
box_adders!(SubTab);

impl Tab {
    pub fn add_sub_tab(&self, info: SubTabInfo) -> SubTab {
        let id = self.ui.mutate(|m| crate::window::tabs::create_sub_tab(m, self.id, info));
        SubTab { ui: self.ui.clone(), id }
    }

    pub fn update_warning_box(&self, info: WarningBoxInfo) {
        self.ui.with(|m| {
            if let Kind::Tab(t) = &mut m.nodes[self.id].kind {
                if let Some(v) = info.is_normal {
                    t.warning.is_normal = v;
                }
                if let Some(v) = info.lock_size {
                    t.warning.lock_size = v;
                }
                if let Some(v) = info.visible {
                    t.warning.visible = v;
                }
                if let Some(v) = info.title {
                    t.warning.title = v;
                }
                if let Some(v) = info.text {
                    t.warning.text = v;
                }
            }
        });
    }

    pub fn add_profile_banner(&self, idx: &str, info: ProfileCardInfo) -> ProfileCard {
        let id = self.ui.mutate(|m| crate::elements::media::create_profile(m, self.id, idx, info, false));
        ProfileCard { ui: self.ui.clone(), id }
    }

    pub fn set_order(&self, order: i32) {
        self.ui.with(|m| {
            if let Kind::Tab(t) = &mut m.nodes[self.id].kind {
                t.order = order;
            }
            crate::window::tabs::sort_tabs(m);
        });
    }

    pub fn is_expanded(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Tab(t) => t.sidebar_expanded,
            _ => false,
        })
    }

    pub fn set_expanded(&self, value: Option<bool>) {
        self.ui.with(|m| {
            if let Kind::Tab(t) = &mut m.nodes[self.id].kind {
                t.sidebar_expanded = value.unwrap_or(!t.sidebar_expanded);
            }
        });
    }

    pub fn toggle_expanded(&self) {
        self.set_expanded(None);
    }

    pub fn set_sub_tab_alignment(&self, align: Align) {
        self.ui.with(|m| {
            if let Kind::Tab(t) = &mut m.nodes[self.id].kind {
                t.sub_tab_align = align;
            }
        });
    }

    pub fn name(&self) -> String {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Tab(t) => t.name.clone(),
            _ => String::new(),
        })
    }
}

impl KeyTab {
    pub fn add_key_box(&self, callback: impl FnMut(&str) + Send + 'static) {
        self.ui.with(|m| {
            let id = m.insert(Kind::KeyBox(KeyBoxData { text: String::new(), callback: vec![Box::new(callback)], focus: Default::default() }), Some(self.id));
            m.add_child(self.id, id);
        });
    }

    pub fn show(&self) {
        self.ui.mutate(|m| crate::window::tabs::show(m, self.id));
    }

    pub fn hide(&self) {
        self.ui.mutate(|m| crate::window::tabs::hide(m, self.id));
    }

    pub fn set_order(&self, order: i32) {
        self.ui.with(|m| {
            if let Kind::KeyTab(t) = &mut m.nodes[self.id].kind {
                t.order = order;
            }
            crate::window::tabs::sort_tabs(m);
        });
    }
}

// ===== Tabbox / Groupbox ====================================================================

impl Tabbox {
    pub fn name(&self) -> Option<String> {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Tabbox(t) => t.name.clone(),
            _ => None,
        })
    }

    pub fn add_tab(&self, name: Option<&str>, icon: Option<IconRef>) -> TabboxTab {
        let id = self.ui.mutate(|m| crate::window::boxes::create_tabbox_tab(m, self.id, name, icon));
        TabboxTab { ui: self.ui.clone(), id }
    }

    pub fn set_popped_out(&self, popped: bool) {
        self.ui.with(|m| crate::window::popout::set_popped_out(m, self.id, popped, None));
    }

    pub fn toggle_popped_out(&self) {
        self.ui.with(|m| {
            let cur = match &m.nodes[self.id].kind {
                Kind::Tabbox(t) => t.pop.popped,
                _ => false,
            };
            crate::window::popout::set_popped_out(m, self.id, !cur, None);
        });
    }
}

impl TabboxTab {
    pub fn show(&self) {
        self.ui.with(|m| crate::window::boxes::show_tabbox_tab(m, self.id));
    }

    pub fn hide(&self) {
        self.ui.with(|m| crate::window::boxes::hide_tabbox_tab(m, self.id));
    }
}

impl Groupbox {
    pub fn add_tabbox(&self, info: TabboxInfo) -> Tabbox {
        let id = self.ui.mutate(|m| crate::window::boxes::create_tabbox(m, self.id, info));
        Tabbox { ui: self.ui.clone(), id }
    }

    pub fn add_dependency_groupbox(&self) -> DependencyGroupbox {
        let id = self.ui.mutate(|m| crate::elements::dependency::create(m, self.id, true));
        DependencyGroupbox { ui: self.ui.clone(), id }
    }

    pub fn set_description(&self, description: Option<&str>) {
        self.ui.with(|m| {
            if let Kind::Groupbox(g) = &mut m.nodes[self.id].kind {
                g.description = description.map(|s| s.to_owned());
            }
        });
    }

    pub fn set_collapsed(&self, collapsed: bool) {
        self.ui.with(|m| {
            if let Kind::Groupbox(g) = &mut m.nodes[self.id].kind {
                if !g.disable_collapsing {
                    g.collapsed = collapsed;
                }
            }
        });
    }

    pub fn toggle_collapsed(&self) {
        self.ui.with(|m| {
            if let Kind::Groupbox(g) = &mut m.nodes[self.id].kind {
                if !g.disable_collapsing {
                    g.collapsed = !g.collapsed;
                }
            }
        });
    }

    pub fn is_collapsed(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Groupbox(g) => g.collapsed,
            _ => false,
        })
    }

    pub fn show(&self) {
        self.set_visible(true);
    }

    pub fn hide(&self) {
        self.set_visible(false);
    }

    pub fn set_popped_out(&self, popped: bool) {
        self.ui.with(|m| crate::window::popout::set_popped_out(m, self.id, popped, None));
    }

    pub fn toggle_popped_out(&self) {
        self.ui.with(|m| {
            let cur = match &m.nodes[self.id].kind {
                Kind::Groupbox(g) => g.pop.popped,
                _ => false,
            };
            crate::window::popout::set_popped_out(m, self.id, !cur, None);
        });
    }

    pub fn is_popped_out(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Groupbox(g) => g.pop.popped,
            _ => false,
        })
    }
}

/// A dependency condition for [`DependencyBox::set_dependencies`].
pub enum Dependency {
    Toggle(Toggle, bool),
    Dropdown(Dropdown, String),
}

macro_rules! dep_handle {
    ($t:ident) => {
        impl $t {
            /// The box is visible only when every condition holds.
            pub fn set_dependencies(&self, deps: Vec<Dependency>) {
                self.ui.mutate(|m| {
                    let deps = deps
                        .into_iter()
                        .map(|d| match d {
                            Dependency::Toggle(t, v) => crate::node::Dependency::Toggle(t.id, v),
                            Dependency::Dropdown(d, v) => crate::node::Dependency::Dropdown(d.id, v),
                        })
                        .collect();
                    if let Kind::DepBox(d) | Kind::DepGroupbox(d) = &mut m.nodes[self.id].kind {
                        d.deps = deps;
                    }
                    crate::elements::dependency::update(m, self.id, false);
                });
            }
        }
    };
}
dep_handle!(DependencyBox);
dep_handle!(DependencyGroupbox);

// ===== Dialog ===============================================================================

impl Dialog {
    pub fn set_title(&self, title: &str) {
        self.ui.with(|m| {
            if let Kind::Dialog(d) = &mut m.nodes[self.id].kind {
                d.title = title.to_owned();
            }
        });
    }

    pub fn set_description(&self, description: &str) {
        self.ui.with(|m| {
            if let Kind::Dialog(d) = &mut m.nodes[self.id].kind {
                d.description = description.to_owned();
            }
        });
    }

    pub fn dismiss(&self) {
        self.ui.with(|m| crate::overlays::dialog::dismiss(m, self.id));
    }

    pub fn add_footer_button(&self, key: &str, info: FooterButtonInfo) {
        self.ui.with(|m| crate::overlays::dialog::add_button(m, self.id, key, info));
    }

    pub fn remove_footer_button(&self, key: &str) {
        self.ui.with(|m| {
            if let Kind::Dialog(d) = &mut m.nodes[self.id].kind {
                d.buttons.retain(|b| b.key != key);
            }
        });
    }

    pub fn set_button_disabled(&self, key: &str, disabled: bool) {
        self.ui.with(|m| {
            if let Kind::Dialog(d) = &mut m.nodes[self.id].kind {
                if let Some(b) = d.buttons.iter_mut().find(|b| b.key == key) {
                    b.disabled = disabled;
                }
            }
        });
    }

    pub fn set_button_order(&self, key: &str, order: i32) {
        self.ui.with(|m| {
            if let Kind::Dialog(d) = &mut m.nodes[self.id].kind {
                if let Some(b) = d.buttons.iter_mut().find(|b| b.key == key) {
                    b.order = order;
                }
                d.buttons.sort_by_key(|b| b.order);
            }
        });
    }
}

// ===== Basic elements =======================================================================

impl Label {
    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Kind::Label(l) = &mut m.nodes[self.id].kind {
                l.text = text.to_owned();
            }
        });
    }

    pub fn add_key_picker(&self, idx: &str, info: KeyPickerInfo) -> KeyPicker {
        let id = self.ui.mutate(|m| crate::elements::keypicker::create(m, self.id, idx, info));
        KeyPicker { ui: self.ui.clone(), id }
    }

    pub fn add_color_picker(&self, idx: &str, info: ColorPickerInfo) -> ColorPicker {
        let id = self.ui.mutate(|m| crate::elements::colorpicker::create(m, self.id, idx, info));
        ColorPicker { ui: self.ui.clone(), id }
    }
}

impl Button {
    /// True for half-width sub-buttons created with [`Button::add_sub_button`].
    pub fn is_sub_button(&self) -> bool {
        self.ui.with(|m| matches!(&m.nodes[self.id].kind, Kind::Button(b) if b.is_sub))
    }

    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Kind::Button(b) = &mut m.nodes[self.id].kind {
                b.text = text.to_owned();
            }
        });
    }

    pub fn set_disabled(&self, disabled: bool) {
        self.ui.with(|m| {
            if let Kind::Button(b) = &mut m.nodes[self.id].kind {
                b.disabled = disabled;
            }
        });
    }

    /// A half-width sub-button sharing the row.
    pub fn add_sub_button(&self, info: ButtonInfo) -> SubButton {
        self.add_sub_button_idx("", info)
    }

    pub fn add_sub_button_idx(&self, idx: &str, info: ButtonInfo) -> SubButton {
        let id = self.ui.mutate(|m| crate::elements::basic::create_button(m, self.id, idx, info, true));
        Button { ui: self.ui.clone(), id }
    }

    /// A Press-mode key picker beside the button.
    pub fn add_key_picker(&self, idx: &str, info: KeyPickerInfo) -> KeyPicker {
        let id = self.ui.mutate(|m| crate::elements::keypicker::create(m, self.id, idx, info));
        KeyPicker { ui: self.ui.clone(), id }
    }

    /// Fire the button's callbacks programmatically.
    pub fn click(&self) {
        self.ui.mutate(|m| m.pending.push(Pending::Unit(self.id)));
    }
}

impl Toggle {
    /// The value given at construction.
    pub fn default_value(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Toggle(t) => t.default,
            _ => false,
        })
    }

    pub fn value(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Toggle(t) => t.value,
            _ => false,
        })
    }

    pub fn set_value(&self, value: bool) {
        self.ui.mutate(|m| crate::elements::toggle::set_value(m, self.id, value));
    }

    pub fn set_disabled(&self, disabled: bool) {
        self.ui.with(|m| {
            if let Kind::Toggle(t) = &mut m.nodes[self.id].kind {
                t.disabled = disabled;
            }
        });
    }

    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Kind::Toggle(t) = &mut m.nodes[self.id].kind {
                t.text = text.to_owned();
            }
        });
    }

    pub fn on_changed(&self, f: impl FnMut(bool) + Send + 'static) {
        self.ui.with(|m| {
            if let Kind::Toggle(t) = &mut m.nodes[self.id].kind {
                t.listeners.push(Box::new(f));
            }
        });
    }

    pub fn variant(&self) -> ToggleVariant {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Toggle(t) => t.variant,
            _ => ToggleVariant::Switch,
        })
    }

    pub fn add_key_picker(&self, idx: &str, info: KeyPickerInfo) -> KeyPicker {
        let id = self.ui.mutate(|m| crate::elements::keypicker::create(m, self.id, idx, info));
        KeyPicker { ui: self.ui.clone(), id }
    }

    pub fn add_color_picker(&self, idx: &str, info: ColorPickerInfo) -> ColorPicker {
        let id = self.ui.mutate(|m| crate::elements::colorpicker::create(m, self.id, idx, info));
        ColorPicker { ui: self.ui.clone(), id }
    }
}

impl Input {
    /// The (pipeline-validated) default value.
    pub fn default_value(&self) -> String {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Input(i) => i.default.clone(),
            _ => String::new(),
        })
    }

    pub fn value(&self) -> String {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Input(i) => i.value.clone(),
            _ => String::new(),
        })
    }

    pub fn set_value(&self, value: &str) {
        self.ui.mutate(|m| crate::elements::input::set_value(m, self.id, value));
    }

    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Kind::Input(i) = &mut m.nodes[self.id].kind {
                i.text = text.to_owned();
            }
        });
    }

    pub fn set_disabled(&self, disabled: bool) {
        self.ui.with(|m| {
            if let Kind::Input(i) = &mut m.nodes[self.id].kind {
                i.disabled = disabled;
            }
        });
    }

    pub fn on_changed(&self, f: impl FnMut(&str) + Send + 'static) {
        self.ui.with(|m| {
            if let Kind::Input(i) = &mut m.nodes[self.id].kind {
                i.listeners.push(Box::new(f));
            }
        });
    }
}

impl Slider {
    pub fn default_value(&self) -> f64 {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Slider(s) => s.default,
            _ => 0.0,
        })
    }

    pub fn value(&self) -> f64 {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Slider(s) => s.value,
            _ => 0.0,
        })
    }

    pub fn set_value(&self, value: f64) {
        self.ui.mutate(|m| crate::elements::slider::set_value(m, self.id, value));
    }

    pub fn set_min(&self, min: f64) {
        self.ui.mutate(|m| crate::elements::slider::set_min(m, self.id, min));
    }

    pub fn set_max(&self, max: f64) {
        self.ui.mutate(|m| crate::elements::slider::set_max(m, self.id, max));
    }

    pub fn set_prefix(&self, prefix: &str) {
        self.ui.with(|m| {
            if let Kind::Slider(s) = &mut m.nodes[self.id].kind {
                s.prefix = prefix.to_owned();
            }
        });
    }

    pub fn set_suffix(&self, suffix: &str) {
        self.ui.with(|m| {
            if let Kind::Slider(s) = &mut m.nodes[self.id].kind {
                s.suffix = suffix.to_owned();
            }
        });
    }

    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Kind::Slider(s) = &mut m.nodes[self.id].kind {
                s.text = text.to_owned();
            }
        });
    }

    pub fn set_disabled(&self, disabled: bool) {
        self.ui.with(|m| {
            if let Kind::Slider(s) = &mut m.nodes[self.id].kind {
                s.disabled = disabled;
            }
        });
    }

    pub fn on_changed(&self, f: impl FnMut(f64) + Send + 'static) {
        self.ui.with(|m| {
            if let Kind::Slider(s) = &mut m.nodes[self.id].kind {
                s.listeners.push(Box::new(f));
            }
        });
    }
}

impl Dropdown {
    /// The selection resolved from `DropdownInfo::default` at construction.
    pub fn default_value(&self) -> DropdownValue {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Dropdown(d) => d.default.clone(),
            _ => DropdownValue::Single(None),
        })
    }

    pub fn value(&self) -> DropdownValue {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Dropdown(d) => d.value.clone(),
            _ => DropdownValue::Single(None),
        })
    }

    /// Selected values as a list.
    pub fn active_values(&self) -> Vec<String> {
        self.value().active_values()
    }

    pub fn set_value(&self, value: DropdownValue) {
        self.ui.mutate(|m| crate::elements::dropdown::set_value(m, self.id, value));
    }

    pub fn set_values(&self, values: Vec<DropdownEntry>) {
        self.ui.mutate(|m| crate::elements::dropdown::set_values(m, self.id, values));
    }

    pub fn add_values(&self, values: Vec<DropdownEntry>) {
        self.ui.with(|m| crate::elements::dropdown::add_values(m, self.id, values));
    }

    pub fn set_disabled_values(&self, values: Vec<String>) {
        self.ui.with(|m| {
            if let Kind::Dropdown(d) = &mut m.nodes[self.id].kind {
                d.disabled_values = values;
                d.filter_dirty = true;
            }
        });
    }

    pub fn add_disabled_values(&self, values: Vec<String>) {
        self.ui.with(|m| {
            if let Kind::Dropdown(d) = &mut m.nodes[self.id].kind {
                d.disabled_values.extend(values);
                d.filter_dirty = true;
            }
        });
    }

    pub fn set_value_images(&self, images: HashMap<String, IconRef>) {
        self.ui.with(|m| {
            if let Kind::Dropdown(d) = &mut m.nodes[self.id].kind {
                d.value_images = images;
            }
        });
    }

    pub fn add_value_images(&self, images: HashMap<String, IconRef>) {
        self.ui.with(|m| {
            if let Kind::Dropdown(d) = &mut m.nodes[self.id].kind {
                d.value_images.extend(images);
            }
        });
    }

    pub fn select_all(&self, search: Option<&str>) {
        self.ui.mutate(|m| crate::elements::dropdown::bulk_select(m, self.id, true, search.unwrap_or("")));
    }

    pub fn deselect_all(&self, search: Option<&str>) {
        self.ui.mutate(|m| crate::elements::dropdown::bulk_select(m, self.id, false, search.unwrap_or("")));
    }

    pub fn set_drag_select(&self, enabled: bool) {
        self.ui.with(|m| {
            if let Kind::Dropdown(d) = &mut m.nodes[self.id].kind {
                d.drag_select = enabled && d.multi;
            }
        });
    }

    pub fn expand(&self) {
        self.ui.with(|m| crate::elements::dropdown::set_expanded(m, self.id, true));
    }

    pub fn collapse(&self) {
        self.ui.with(|m| crate::elements::dropdown::set_expanded(m, self.id, false));
    }

    pub fn is_expanded(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Dropdown(d) => d.expanded,
            _ => false,
        })
    }

    pub fn toggle_expanded(&self) {
        let v = self.is_expanded();
        self.ui.with(|m| crate::elements::dropdown::set_expanded(m, self.id, !v));
    }

    pub fn set_text(&self, text: Option<&str>) {
        self.ui.with(|m| {
            if let Kind::Dropdown(d) = &mut m.nodes[self.id].kind {
                d.text = text.map(|s| s.to_owned());
            }
        });
    }

    pub fn set_disabled(&self, disabled: bool) {
        self.ui.with(|m| crate::elements::dropdown::set_disabled(m, self.id, disabled));
    }

    pub fn on_changed(&self, f: impl FnMut(&DropdownValue) + Send + 'static) {
        self.ui.with(|m| {
            if let Kind::Dropdown(d) = &mut m.nodes[self.id].kind {
                d.listeners.push(Box::new(f));
            }
        });
    }
}

impl PriorityDropdown {
    pub fn default_value(&self) -> Vec<String> {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Priority(p) => p.default.clone(),
            _ => Vec::new(),
        })
    }

    pub fn value(&self) -> Vec<String> {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Priority(p) => p.value.clone(),
            _ => Vec::new(),
        })
    }

    pub fn set_value(&self, order: Vec<String>) {
        self.ui.mutate(|m| crate::elements::priority::set_value(m, self.id, order));
    }

    pub fn set_values(&self, values: Vec<String>) {
        self.ui.mutate(|m| crate::elements::priority::set_values(m, self.id, values));
    }

    pub fn expand(&self) {
        self.ui.with(|m| crate::elements::priority::set_expanded(m, self.id, true));
    }

    pub fn collapse(&self) {
        self.ui.with(|m| crate::elements::priority::set_expanded(m, self.id, false));
    }

    pub fn is_expanded(&self) -> bool {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Priority(p) => p.expanded,
            _ => false,
        })
    }

    pub fn toggle_expanded(&self) {
        let v = self.is_expanded();
        self.ui.with(|m| crate::elements::priority::set_expanded(m, self.id, !v));
    }

    pub fn set_text(&self, text: Option<&str>) {
        self.ui.with(|m| {
            if let Kind::Priority(p) = &mut m.nodes[self.id].kind {
                p.text = text.map(|s| s.to_owned());
            }
        });
    }

    pub fn set_disabled(&self, disabled: bool) {
        self.ui.with(|m| crate::elements::priority::set_disabled(m, self.id, disabled));
    }

    pub fn on_changed(&self, f: impl FnMut(&[String]) + Send + 'static) {
        self.ui.with(|m| {
            if let Kind::Priority(p) = &mut m.nodes[self.id].kind {
                p.listeners.push(Box::new(f));
            }
        });
    }
}

impl Image {
    pub fn set_image(&self, image: IconRef) {
        self.ui.with(|m| {
            if let Kind::Image(i) = &mut m.nodes[self.id].kind {
                i.image = image;
            }
        });
    }
    pub fn set_height(&self, height: f32) {
        self.ui.with(|m| {
            if let Kind::Image(i) = &mut m.nodes[self.id].kind {
                i.height = height.max(1.0);
            }
        });
    }
    pub fn set_color(&self, color: Color32) {
        self.ui.with(|m| {
            if let Kind::Image(i) = &mut m.nodes[self.id].kind {
                i.color = color;
            }
        });
    }
    pub fn set_uv(&self, uv: Option<Rect>) {
        self.ui.with(|m| {
            if let Kind::Image(i) = &mut m.nodes[self.id].kind {
                i.uv = uv;
            }
        });
    }
    pub fn set_scale_type(&self, scale_type: ScaleType) {
        self.ui.with(|m| {
            if let Kind::Image(i) = &mut m.nodes[self.id].kind {
                i.scale_type = scale_type;
            }
        });
    }
    pub fn set_transparency(&self, transparency: f32) {
        self.ui.with(|m| {
            if let Kind::Image(i) = &mut m.nodes[self.id].kind {
                i.transparency = transparency.clamp(0.0, 1.0);
            }
        });
    }
}

impl ProfileCard {
    pub fn set_title(&self, title: &str) {
        self.ui.with(|m| {
            if let Kind::ProfileCard(p) = &mut m.nodes[self.id].kind {
                p.title = title.to_owned();
            }
        });
    }
    pub fn set_description(&self, description: Description) {
        self.ui.with(|m| {
            if let Kind::ProfileCard(p) = &mut m.nodes[self.id].kind {
                p.description = description;
            }
        });
    }
    pub fn set_avatar(&self, avatar: Option<IconRef>) {
        self.ui.with(|m| {
            if let Kind::ProfileCard(p) = &mut m.nodes[self.id].kind {
                p.avatar = avatar;
            }
        });
    }
    pub fn set_name(&self, name: &str) {
        self.ui.with(|m| {
            if let Kind::ProfileCard(p) = &mut m.nodes[self.id].kind {
                p.name = name.to_owned();
            }
        });
    }
    pub fn set_height(&self, height: f32) {
        self.ui.with(|m| {
            if let Kind::ProfileCard(p) = &mut m.nodes[self.id].kind {
                p.height = Some(height.max(1.0));
            }
        });
    }
}

impl Custom {
    pub fn set_height(&self, height: f32) {
        self.ui.with(|m| {
            if let Kind::Custom(c) = &mut m.nodes[self.id].kind {
                c.height = height.max(1.0);
            }
        });
    }
}

impl KeyPicker {
    pub fn default_value(&self) -> KeyBind {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::KeyPicker(k) => k.default.clone(),
            _ => KeyBind::default(),
        })
    }

    pub fn value(&self) -> KeyBind {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::KeyPicker(k) => k.value.clone(),
            _ => KeyBind::default(),
        })
    }

    pub fn set_value(&self, value: KeyBind) {
        self.ui.mutate(|m| crate::elements::keypicker::set_value(m, self.id, value));
    }

    /// Always -> true; Hold -> key held; Toggle/Press -> toggled.
    pub fn state(&self) -> bool {
        self.ui.with(|m| crate::elements::keypicker::state(m, self.id))
    }

    pub fn mode(&self) -> KeyMode {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::KeyPicker(k) => k.value.mode,
            _ => KeyMode::Toggle,
        })
    }

    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Kind::KeyPicker(k) = &mut m.nodes[self.id].kind {
                k.text = text.to_owned();
            }
        });
    }

    pub fn on_changed(&self, f: impl FnMut(&KeyBind) + Send + 'static) {
        self.ui.with(|m| {
            if let Kind::KeyPicker(k) = &mut m.nodes[self.id].kind {
                k.changed.push(Box::new(f));
            }
        });
    }

    pub fn on_click(&self, f: impl FnMut(bool) + Send + 'static) {
        self.ui.with(|m| {
            if let Kind::KeyPicker(k) = &mut m.nodes[self.id].kind {
                k.clicked.push(Box::new(f));
            }
        });
    }

    /// Fire the picker as if its key were pressed.
    pub fn do_click(&self) {
        self.ui.mutate(|m| crate::elements::keypicker::do_click(m, self.id));
    }
}

impl ColorPicker {
    pub fn default_value(&self) -> Rgba {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::ColorPicker(c) => c.default,
            _ => Rgba::opaque(Color32::WHITE),
        })
    }

    pub fn value(&self) -> Rgba {
        self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::ColorPicker(c) => c.value(),
            _ => Rgba::opaque(Color32::WHITE),
        })
    }

    pub fn set_value(&self, value: Rgba) {
        self.ui.mutate(|m| crate::elements::colorpicker::set_value(m, self.id, value.color, Some(value.transparency)));
    }

    pub fn set_color(&self, color: Color32) {
        self.ui.mutate(|m| crate::elements::colorpicker::set_value(m, self.id, color, None));
    }

    pub fn set_hsv(&self, h: f32, s: f32, v: f32, transparency: Option<f32>) {
        self.ui.mutate(|m| crate::elements::colorpicker::set_hsv(m, self.id, h, s, v, transparency));
    }

    pub fn on_changed(&self, f: impl FnMut(Rgba) + Send + 'static) {
        self.ui.with(|m| {
            if let Kind::ColorPicker(c) = &mut m.nodes[self.id].kind {
                c.listeners.push(Box::new(f));
            }
        });
    }
}

// ===== Notifications / loading / floats =====================================================

impl Notification {
    pub fn set_title(&self, title: &str) {
        self.ui.with(|m| {
            if let Some(Kind::Notification(n)) = m.nodes.get_mut(self.id).map(|n| &mut n.kind) {
                n.title = Some(title.to_owned());
            }
        });
    }
    pub fn set_description(&self, description: &str) {
        self.ui.with(|m| {
            if let Some(Kind::Notification(n)) = m.nodes.get_mut(self.id).map(|n| &mut n.kind) {
                n.description = description.to_owned();
            }
        });
    }
    pub fn set_step(&self, step: u32) {
        self.ui.with(|m| {
            if let Some(Kind::Notification(n)) = m.nodes.get_mut(self.id).map(|n| &mut n.kind) {
                if let Some(total) = n.steps {
                    n.step = step.min(total);
                }
            }
        });
    }
}

impl Loading {
    pub fn set_message(&self, text: &str) {
        self.with_data(|l| l.message = text.to_owned());
    }
    pub fn set_description(&self, text: &str) {
        self.with_data(|l| l.description = text.to_owned());
    }
    pub fn set_loading_icon(&self, icon: IconRef) {
        self.with_data(|l| l.loading_icon = icon);
    }
    pub fn set_loading_icon_tween_time(&self, secs: f32) {
        self.with_data(|l| l.spin_time = secs.max(0.0));
    }
    pub fn set_loading_icon_color(&self, color: Option<Color32>) {
        self.with_data(|l| l.loading_icon_color = color);
    }
    pub fn set_current_step(&self, step: u32) {
        self.with_data(|l| l.current_step = step.min(l.total_steps));
    }
    pub fn set_total_steps(&self, total: u32) {
        self.with_data(|l| {
            l.total_steps = total.max(1);
            l.current_step = l.current_step.min(l.total_steps);
        });
    }
    pub fn set_window_height(&self, h: f32) {
        self.with_data(|l| {
            l.window_height = h.max(1.0);
            l.base_height = l.window_height;
        });
    }
    pub fn set_window_width(&self, w: f32) {
        self.with_data(|l| l.window_width = w.max(1.0));
    }
    pub fn set_content_width(&self, w: f32) {
        self.with_data(|l| l.content_width = w.max(1.0));
    }
    pub fn set_sidebar_width(&self, w: f32) {
        self.with_data(|l| l.sidebar_width = w.max(0.0));
    }
    pub fn show_sidebar_page(&self, show: bool) {
        self.with_data(|l| l.show_sidebar = show);
    }
    pub fn show_error_page(&self, enabled: bool) {
        self.with_data(|l| {
            l.is_error = enabled;
            if l.info.show_sidebar {
                l.show_sidebar = !enabled;
            }
        });
    }
    pub fn set_error_message(&self, text: &str) {
        self.with_data(|l| l.error_message = text.to_owned());
    }
    pub fn set_error_buttons(&self, buttons: Vec<(String, ButtonVariant, Option<LoadingFn>)>) {
        self.with_data(|l| {
            l.error_buttons = buttons
                .into_iter()
                .map(|(title, variant, cb)| LoadingButton { title, variant, callback: cb.into_iter().collect(), hover: Default::default(), rect: Rect::NOTHING })
                .collect();
        });
    }
    pub fn sidebar(&self) -> LoadingSidebar {
        let id = self.ui.with(|m| match &m.nodes[self.id].kind {
            Kind::Loading(l) => l.sidebar,
            _ => self.id,
        });
        LoadingSidebar { ui: self.ui.clone(), id }
    }
    /// Same as `destroy`.
    pub fn continue_(&self) {
        self.destroy();
    }

    fn with_data(&self, f: impl FnOnce(&mut LoadingData)) {
        self.ui.with(|m| {
            if let Some(Kind::Loading(l)) = m.nodes.get_mut(self.id).map(|n| &mut n.kind) {
                f(l);
            }
        });
    }
}

impl Watermark {
    pub fn refresh(&self) {
        self.ui.with(|m| crate::overlays::floats::refresh_watermark(m, self.id));
    }
    pub fn set_segments(&self, segments: Vec<WatermarkSegment>) {
        self.ui.with(|m| {
            if let Kind::Watermark(w) = &mut m.nodes[self.id].kind {
                w.segments = segments;
                w.texts.clear();
            }
            crate::overlays::floats::refresh_watermark(m, self.id);
        });
    }
    pub fn set_text(&self, index: usize, text: &str) {
        self.ui.with(|m| {
            if let Kind::Watermark(w) = &mut m.nodes[self.id].kind {
                if let Some(t) = w.texts.get_mut(index) {
                    *t = text.to_owned();
                }
                if let Some(seg) = w.segments.get_mut(index) {
                    seg.text = Some(text.to_owned());
                    seg.getter = None;
                }
            }
        });
    }
    /// A single text segment.
    pub fn set_single_text(&self, text: &str) {
        self.set_segments(vec![WatermarkSegment::text(text)]);
    }
    pub fn set_refresh_rate(&self, secs: f32) {
        self.ui.with(|m| {
            if let Kind::Watermark(w) = &mut m.nodes[self.id].kind {
                w.refresh_rate = secs.max(0.05);
            }
        });
    }
}

impl DraggableLabel {
    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Kind::DraggableLabel(l) = &mut m.nodes[self.id].kind {
                l.text = text.to_owned();
            }
        });
    }
    pub fn set_icon(&self, icon: Option<IconRef>) {
        self.ui.with(|m| {
            if let Kind::DraggableLabel(l) = &mut m.nodes[self.id].kind {
                l.icon = icon;
            }
        });
    }
    pub fn set_icon_position(&self, position: IconPosition) {
        self.ui.with(|m| {
            if let Kind::DraggableLabel(l) = &mut m.nodes[self.id].kind {
                l.icon_position = position;
            }
        });
    }
}

impl DraggableButton {
    pub fn set_text(&self, text: &str) {
        self.ui.with(|m| {
            if let Kind::DraggableButton(b) = &mut m.nodes[self.id].kind {
                b.text = text.to_owned();
            }
        });
    }
}

impl DraggableImageButton {
    pub fn set_icon(&self, icon: IconRef) {
        self.ui.with(|m| {
            if let Kind::DraggableImageButton(b) = &mut m.nodes[self.id].kind {
                b.icon = icon;
            }
        });
    }
    pub fn set_icon_size(&self, size: f32) {
        self.ui.with(|m| {
            if let Kind::DraggableImageButton(b) = &mut m.nodes[self.id].kind {
                b.icon_size = size.max(1.0);
            }
        });
    }
}

/// Screen returned by [`Ui::create_unsupported_screen`].
#[derive(Clone, Debug)]
pub struct UnsupportedScreen {
    pub(crate) ui: Ui,
    pub executor: String,
    pub window: Window,
}

impl UnsupportedScreen {
    /// Unloads the whole UI.
    pub fn destroy(&self) {
        self.ui.unload();
    }
}
