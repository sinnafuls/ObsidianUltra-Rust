//! The library root (`Library` in Lua): owns the model, settings, registries,
//! runs frames and dispatches listeners.

use std::collections::HashMap;
use std::sync::Arc;

use epaint::{Color32, Pos2, Rect};
use parking_lot::Mutex;
use slotmap::SlotMap;

use crate::color::Rgba;
use crate::frame::{Cx, FrameInput, FrameOutput, Layer};
pub use crate::handles::info::Animations;
use crate::handles::*;
use crate::input::Key;
use crate::layout::M;
use crate::node::*;
use crate::scheme::Scheme;
use crate::tween::Anim;
use crate::types::*;

/// Library-wide settings (`Library.*` option fields).
#[derive(Debug, Clone)]
pub struct UiSettings {
    pub toggle_keybind: Option<Key>,
    /// A key picker whose click also toggles the UI.
    pub toggle_keybind_picker: Option<NodeId>,
    pub show_toggle_frame_in_keybinds: bool,
    pub notify_on_error: bool,
    pub show_custom_cursor: bool,
    pub force_checkbox: bool,
    pub global_search: bool,
    pub fuzzy_search: bool,
    pub search_values: bool,
    pub cant_drag_forced: bool,
    pub pop_out_snap_distance: f32,
    pub pop_out_drag_threshold: f32,
    pub pop_out_hold_time: f32,
    pub animations: Animations,
    pub notify_side: Side,
    pub notification_history_limit: usize,
    pub notification_history_keybind: Option<Key>,
    pub notification_type_colors: [(NotifyType, Color32); 4],
    pub unlock_mouse_while_open: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            toggle_keybind: Some(Key::RightControl),
            toggle_keybind_picker: None,
            show_toggle_frame_in_keybinds: true,
            notify_on_error: false,
            show_custom_cursor: true,
            force_checkbox: false,
            global_search: false,
            fuzzy_search: true,
            search_values: true,
            cant_drag_forced: false,
            pop_out_snap_distance: 80.0,
            pop_out_drag_threshold: 8.0,
            pop_out_hold_time: 0.15,
            animations: Animations::default(),
            notify_side: Side::Right,
            notification_history_limit: 100,
            notification_history_keybind: Some(Key::RightAlt),
            notification_type_colors: [
                (NotifyType::Error, Color32::from_rgb(255, 76, 76)),
                (NotifyType::Warning, Color32::from_rgb(255, 176, 32)),
                (NotifyType::Success, Color32::from_rgb(96, 216, 118)),
                (NotifyType::Info, Color32::from_rgb(96, 165, 255)),
            ],
            unlock_mouse_while_open: true,
        }
    }
}

impl UiSettings {
    pub fn type_color(&self, t: NotifyType) -> Color32 {
        self.notification_type_colors.iter().find(|(k, _)| *k == t).map(|(_, c)| *c).unwrap_or(Color32::WHITE)
    }
}

/// One notification-history entry.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub title: Option<String>,
    pub description: String,
    pub title_color: Option<Color32>,
    pub description_color: Option<Color32>,
    pub icon: Option<IconRef>,
    pub icon_color: Option<Color32>,
    pub kind: Option<NotifyType>,
    /// Seconds since the Unix epoch (UTC).
    pub timestamp: u64,
    /// `HH:MM:SS` (UTC).
    pub time_string: String,
    /// (until, ok) copy feedback state.
    pub copied: Option<(f64, bool)>,
    pub hover: Anim,
}

/// Deferred listener invocations; drained outside the model lock.
pub(crate) enum Pending {
    Bool(NodeId, bool),
    Number(NodeId, f64),
    Text(NodeId, String),
    Dropdown(NodeId, DropdownValue),
    List(NodeId, Vec<String>),
    Key(NodeId, KeyBind),
    KeyClick(NodeId, bool),
    Color(NodeId, Rgba),
    Unit(NodeId),
    DialogButton(NodeId, String),
    ErrorButton(NodeId, usize),
    ToggleUi,
}

/// Tooltip state (single shared label).
#[derive(Clone, Debug, Default)]
pub(crate) struct TooltipState {
    pub node: Option<NodeId>,
    pub text: String,
    pub anim: Anim,
    pub visible: bool,
    pub hide_at: f64,
}

/// The whole retained model.
pub(crate) struct Model {
    pub nodes: SlotMap<NodeId, Node>,
    pub window: Option<NodeId>,
    pub scheme: Scheme,
    pub settings: UiSettings,
    pub scale: f32,
    pub corner_radius: f32,
    pub open: bool,
    pub unloaded: bool,
    pub toggles: HashMap<String, NodeId>,
    pub options: HashMap<String, NodeId>,
    pub labels: Vec<NodeId>,
    pub buttons: Vec<NodeId>,
    pub pending: Vec<Pending>,
    pub unload_callbacks: Vec<Box<dyn FnOnce() + Send>>,
    pub notifications: Vec<NodeId>,
    pub history: Vec<HistoryEntry>,
    pub history_open: bool,
    pub history_pos: Pos2,
    pub history_rest: Pos2,
    pub history_anim: Anim,
    pub history_visible: bool,
    pub history_unread: usize,
    pub history_scroll: crate::layout::Scroll,
    pub history_hide_at: f64,
    /// Floats in z order (last = topmost): draggable widgets, watermark, keybind frame, pop-outs.
    pub floats: Vec<NodeId>,
    pub next_z: u32,
    pub current_menu: Option<(NodeId, u8)>,
    pub active_dialog: Option<NodeId>,
    pub active_expanded: Option<NodeId>,
    pub loading: Option<NodeId>,
    pub tooltip: TooltipState,
    pub search_text: String,
    pub searching: bool,
    pub last_search_tab: Option<NodeId>,
    pub dep_boxes: Vec<NodeId>,
    pub keybind_frame: Option<NodeId>,
    pub key_pickers: Vec<NodeId>,
    pub copied_color: Option<Rgba>,
    pub capture: Option<Capture>,
    pub cursor_icon: Option<IconRef>,
    pub cursor_icon_size: f32,
    pub watermark: Option<NodeId>,
    pub window_fade: Anim,
    pub fading: bool,
    pub time: f64,
    pub screen: Rect,
    pub ppp: f32,
    /// Overlay rects from the last frame, used for pointer blocking (layer, rect).
    pub overlays: Vec<(Layer, Rect)>,
    /// Overlay rects of the previous frame (text-field coverage checks).
    pub prev_overlays: Vec<(Layer, Rect)>,
    pub picking: bool,
    pub first_frame_done: bool,
    pub icons_needed: Vec<(IconRef, u32)>,
}

impl Model {
    fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            window: None,
            scheme: Scheme::default(),
            settings: UiSettings::default(),
            scale: 1.0,
            corner_radius: 4.0,
            open: false,
            unloaded: false,
            toggles: HashMap::new(),
            options: HashMap::new(),
            labels: Vec::new(),
            buttons: Vec::new(),
            pending: Vec::new(),
            unload_callbacks: Vec::new(),
            notifications: Vec::new(),
            history: Vec::new(),
            history_open: false,
            history_pos: Pos2::ZERO,
            history_rest: Pos2::ZERO,
            history_anim: Anim::new(0.0),
            history_visible: false,
            history_unread: 0,
            history_scroll: Default::default(),
            history_hide_at: 0.0,
            floats: Vec::new(),
            next_z: 1,
            current_menu: None,
            active_dialog: None,
            active_expanded: None,
            loading: None,
            tooltip: TooltipState::default(),
            search_text: String::new(),
            searching: false,
            last_search_tab: None,
            dep_boxes: Vec::new(),
            keybind_frame: None,
            key_pickers: Vec::new(),
            copied_color: None,
            capture: None,
            cursor_icon: None,
            cursor_icon_size: 20.0,
            watermark: None,
            window_fade: Anim::new(1.0),
            fading: false,
            time: 0.0,
            screen: Rect::from_min_size(Pos2::ZERO, epaint::Vec2::new(1920.0, 1080.0)),
            ppp: 1.0,
            overlays: Vec::new(),
            prev_overlays: Vec::new(),
            picking: false,
            first_frame_done: false,
            icons_needed: Vec::new(),
        }
    }


    pub fn alive(&self, id: NodeId) -> bool {
        self.nodes.get(id).map(|n| !n.destroyed).unwrap_or(false)
    }

    pub fn insert(&mut self, kind: Kind, parent: Option<NodeId>) -> NodeId {
        self.nodes.insert(Node::new(kind, parent))
    }

    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[parent].children.push(child);
    }

    pub fn bump_z(&mut self) -> u32 {
        self.next_z += 1;
        self.next_z
    }

    /// Bring a float to the top of the float stack.
    pub fn raise_float(&mut self, id: NodeId) {
        if let Some(pos) = self.floats.iter().position(|f| *f == id) {
            self.floats.remove(pos);
            self.floats.push(id);
        }
    }

    pub fn register_option(&mut self, idx: &str, id: NodeId) {
        if !idx.is_empty() {
            self.options.insert(idx.to_owned(), id);
            self.nodes[id].idx = idx.to_owned();
        }
    }

    pub fn register_toggle(&mut self, idx: &str, id: NodeId) {
        if !idx.is_empty() {
            self.toggles.insert(idx.to_owned(), id);
            self.nodes[id].idx = idx.to_owned();
        }
    }

    /// Recursively mark a subtree destroyed and drop it from registries and lists.
    pub fn destroy_subtree(&mut self, id: NodeId) {
        let Some(node) = self.nodes.get(id) else { return };
        if node.destroyed {
            return;
        }
        let children = node.children.clone();
        let extra: Vec<NodeId> = match &node.kind {
            Kind::Tab(t) => t.boxes.iter().chain(t.sub_tabs.iter()).chain(t.banners.iter()).copied().collect(),
            Kind::SubTab(t) => t.boxes.clone(),
            Kind::Tabbox(t) => t.tabs.clone(),
            Kind::Label(l) => l.addons.clone(),
            Kind::Toggle(t) => t.addons.clone(),
            Kind::Button(b) => b.addons.iter().copied().chain(b.sub).collect(),
            Kind::Window(w) => w.tabs.iter().chain(w.dialogs.iter()).chain(w.mini_labels.iter()).copied().collect(),
            Kind::Loading(l) => vec![l.sidebar],
            _ => Vec::new(),
        };
        for c in children.into_iter().chain(extra) {
            self.destroy_subtree(c);
        }
        let node = &mut self.nodes[id];
        node.destroyed = true;
        let idx = std::mem::take(&mut node.idx);
        if !idx.is_empty() {
            if self.toggles.get(&idx) == Some(&id) {
                self.toggles.remove(&idx);
            }
            if self.options.get(&idx) == Some(&id) {
                self.options.remove(&idx);
            }
        }
        self.labels.retain(|n| *n != id);
        self.buttons.retain(|n| *n != id);
        self.floats.retain(|n| *n != id);
        self.dep_boxes.retain(|n| *n != id);
        self.key_pickers.retain(|n| *n != id);
        self.notifications.retain(|n| *n != id);
        if self.current_menu.map(|(n, _)| n) == Some(id) {
            self.current_menu = None;
        }
        if self.active_dialog == Some(id) {
            self.active_dialog = None;
        }
        if self.active_expanded == Some(id) {
            self.active_expanded = None;
        }
        if self.capture.map(|c| c.node) == Some(id) {
            self.capture = None;
        }
        if self.tooltip.node == Some(id) {
            self.tooltip.node = None;
            self.tooltip.visible = false;
        }
        // Detach from the parent's child lists.
        if let Some(parent) = self.nodes[id].parent {
            if let Some(p) = self.nodes.get_mut(parent) {
                p.children.retain(|c| *c != id);
                match &mut p.kind {
                    Kind::Tab(t) => {
                        t.boxes.retain(|c| *c != id);
                        t.sub_tabs.retain(|c| *c != id);
                        t.banners.retain(|c| *c != id);
                        if t.active_sub_tab == Some(id) {
                            t.active_sub_tab = None;
                        }
                    }
                    Kind::SubTab(t) => t.boxes.retain(|c| *c != id),
                    Kind::Tabbox(t) => {
                        t.tabs.retain(|c| *c != id);
                        if t.active == Some(id) {
                            t.active = t.tabs.first().copied();
                        }
                    }
                    Kind::Label(l) => l.addons.retain(|c| *c != id),
                    Kind::Toggle(t) => t.addons.retain(|c| *c != id),
                    Kind::Button(b) => {
                        b.addons.retain(|c| *c != id);
                        if b.sub == Some(id) {
                            b.sub = None;
                        }
                    }
                    Kind::Window(w) => {
                        w.tabs.retain(|c| *c != id);
                        w.dialogs.retain(|c| *c != id);
                        w.mini_labels.retain(|c| *c != id);
                        if w.active_tab == Some(id) {
                            w.active_tab = None;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    pub fn update_dependency_boxes(&mut self) {
        let boxes = self.dep_boxes.clone();
        for id in boxes {
            crate::elements::dependency::update(self, id, true);
        }
        if self.searching {
            let text = self.search_text.clone();
            crate::overlays::search::update_search(self, &text);
        }
    }

    pub fn push_history(&mut self, entry: HistoryEntry) {
        self.history.insert(0, entry);
        let limit = self.settings.notification_history_limit.max(1);
        self.history.truncate(limit);
        if !self.history_open {
            self.history_unread += 1;
        }
    }
}

/// The library root. Cheap to clone; all clones share one model.
#[derive(Clone)]
pub struct Ui {
    pub(crate) model: Arc<Mutex<Model>>,
}

impl std::fmt::Debug for Ui {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Ui")
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        let ui = Self { model: Arc::new(Mutex::new(Model::new())) };
        ui.with(|m| {
            // Built-in hidden watermark.
            let id = crate::overlays::floats::create_watermark(m, Vec::new());
            m.nodes[id].visible = false;
            m.watermark = Some(id);
            // Keybind frame.
            let kf = crate::overlays::floats::create_draggable_menu(m, "Keybinds", true);
            m.keybind_frame = Some(kf);
        });
        ui
    }

    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut Model) -> R) -> R {
        let mut m = self.model.lock();
        f(&mut m)
    }

    /// Lock, mutate, then run any listeners queued by the mutation.
    pub(crate) fn mutate<R>(&self, f: impl FnOnce(&mut Model) -> R) -> R {
        let r = self.with(f);
        self.flush_pending();
        r
    }

    /// Drain queued listener invocations without holding the model lock while they run.
    pub(crate) fn flush_pending(&self) {
        loop {
            let Some(p) = self.with(|m| if m.pending.is_empty() { None } else { Some(m.pending.remove(0)) }) else { break };
            self.dispatch(p);
        }
    }

    fn dispatch(&self, p: Pending) {
        match p {
            Pending::Bool(id, v) => {
                let ls = self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::Toggle(t)) => std::mem::take(&mut t.listeners),
                    _ => Vec::new(),
                });
                let ls = self.run_all(ls, |f| f(v));
                self.with(|m| {
                    if let Some(Kind::Toggle(t)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
                        let new = std::mem::replace(&mut t.listeners, ls);
                        t.listeners.extend(new);
                    }
                });
            }
            Pending::Number(id, v) => {
                let ls = self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::Slider(s)) => std::mem::take(&mut s.listeners),
                    _ => Vec::new(),
                });
                let ls = self.run_all(ls, |f| f(v));
                self.with(|m| {
                    if let Some(Kind::Slider(s)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
                        let new = std::mem::replace(&mut s.listeners, ls);
                        s.listeners.extend(new);
                    }
                });
            }
            Pending::Text(id, v) => {
                let ls = self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::Input(i)) => std::mem::take(&mut i.listeners),
                    Some(Kind::KeyBox(k)) => std::mem::take(&mut k.callback),
                    _ => Vec::new(),
                });
                let ls = self.run_all(ls, |f| f(&v));
                self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::Input(i)) => {
                        let new = std::mem::replace(&mut i.listeners, ls);
                        i.listeners.extend(new);
                    }
                    Some(Kind::KeyBox(k)) => {
                        let new = std::mem::replace(&mut k.callback, ls);
                        k.callback.extend(new);
                    }
                    _ => {}
                });
            }
            Pending::Dropdown(id, v) => {
                let ls = self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::Dropdown(d)) => std::mem::take(&mut d.listeners),
                    _ => Vec::new(),
                });
                let ls = self.run_all(ls, |f| f(&v));
                self.with(|m| {
                    if let Some(Kind::Dropdown(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
                        let new = std::mem::replace(&mut d.listeners, ls);
                        d.listeners.extend(new);
                    }
                });
            }
            Pending::List(id, v) => {
                let ls = self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::Priority(d)) => std::mem::take(&mut d.listeners),
                    _ => Vec::new(),
                });
                let ls = self.run_all(ls, |f| f(&v));
                self.with(|m| {
                    if let Some(Kind::Priority(d)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
                        let new = std::mem::replace(&mut d.listeners, ls);
                        d.listeners.extend(new);
                    }
                });
            }
            Pending::Key(id, v) => {
                let ls = self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::KeyPicker(k)) => std::mem::take(&mut k.changed),
                    _ => Vec::new(),
                });
                let ls = self.run_all(ls, |f| f(&v));
                self.with(|m| {
                    if let Some(Kind::KeyPicker(k)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
                        let new = std::mem::replace(&mut k.changed, ls);
                        k.changed.extend(new);
                    }
                });
            }
            Pending::KeyClick(id, toggled) => {
                let (cb, cl, host, host_button, press, is_toggle_key) = self.with(|m| {
                    let is_toggle_key = m.settings.toggle_keybind_picker == Some(id);
                    match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                        Some(Kind::KeyPicker(k)) => (
                            std::mem::take(&mut k.callback),
                            std::mem::take(&mut k.clicked),
                            k.host,
                            k.host_is_button,
                            k.value.mode == KeyMode::Press,
                            is_toggle_key,
                        ),
                        _ => (Vec::new(), Vec::new(), id, false, false, false),
                    }
                });
                let cb = self.run_all(cb, |f| f(toggled));
                let cl = self.run_all(cl, |f| f(toggled));
                self.with(|m| {
                    if let Some(Kind::KeyPicker(k)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
                        let new = std::mem::replace(&mut k.callback, cb);
                        k.callback.extend(new);
                        let new = std::mem::replace(&mut k.clicked, cl);
                        k.clicked.extend(new);
                    }
                    if host_button && press {
                        m.pending.push(Pending::Unit(host));
                    }
                    if is_toggle_key {
                        m.pending.push(Pending::ToggleUi);
                    }
                });
            }
            Pending::Color(id, v) => {
                let ls = self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::ColorPicker(c)) => std::mem::take(&mut c.listeners),
                    _ => Vec::new(),
                });
                let ls = self.run_all(ls, |f| f(v));
                self.with(|m| {
                    if let Some(Kind::ColorPicker(c)) = m.nodes.get_mut(id).map(|n| &mut n.kind) {
                        let new = std::mem::replace(&mut c.listeners, ls);
                        c.listeners.extend(new);
                    }
                });
            }
            Pending::Unit(id) => {
                let ls = self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::Button(b)) => std::mem::take(&mut b.func),
                    Some(Kind::DraggableButton(b)) => std::mem::take(&mut b.callback),
                    Some(Kind::DraggableImageButton(b)) => std::mem::take(&mut b.callback),
                    _ => Vec::new(),
                });
                let ls = self.run_all(ls, |f| f());
                self.with(|m| match m.nodes.get_mut(id).map(|n| &mut n.kind) {
                    Some(Kind::Button(b)) => {
                        let new = std::mem::replace(&mut b.func, ls);
                        b.func.extend(new);
                    }
                    Some(Kind::DraggableButton(b)) => {
                        let new = std::mem::replace(&mut b.callback, ls);
                        b.callback.extend(new);
                    }
                    Some(Kind::DraggableImageButton(b)) => {
                        let new = std::mem::replace(&mut b.callback, ls);
                        b.callback.extend(new);
                    }
                    _ => {}
                });
            }
            Pending::DialogButton(dialog, key) => {
                let ls = self.with(|m| match m.nodes.get_mut(dialog).map(|n| &mut n.kind) {
                    Some(Kind::Dialog(d)) => d.buttons.iter_mut().find(|b| b.key == key).map(|b| std::mem::take(&mut b.callback)).unwrap_or_default(),
                    _ => Vec::new(),
                });
                let handle = Dialog { ui: self.clone(), id: dialog };
                let ls = self.run_all(ls, |f| f(&handle));
                self.with(|m| {
                    if let Some(Kind::Dialog(d)) = m.nodes.get_mut(dialog).map(|n| &mut n.kind) {
                        if let Some(b) = d.buttons.iter_mut().find(|b| b.key == key) {
                            let new = std::mem::replace(&mut b.callback, ls);
                            b.callback.extend(new);
                        }
                    }
                });
            }
            Pending::ErrorButton(loading, index) => {
                let ls = self.with(|m| match m.nodes.get_mut(loading).map(|n| &mut n.kind) {
                    Some(Kind::Loading(l)) => l.error_buttons.get_mut(index).map(|b| std::mem::take(&mut b.callback)).unwrap_or_default(),
                    _ => Vec::new(),
                });
                let handle = Loading { ui: self.clone(), id: loading };
                let ls = self.run_all(ls, |f| f(&handle));
                self.with(|m| {
                    if let Some(Kind::Loading(l)) = m.nodes.get_mut(loading).map(|n| &mut n.kind) {
                        if let Some(b) = l.error_buttons.get_mut(index) {
                            let new = std::mem::replace(&mut b.callback, ls);
                            b.callback.extend(new);
                        }
                    }
                });
            }
            Pending::ToggleUi => self.toggle(None),
        }
    }

    /// Run listeners outside the lock, catching panics.
    fn run_all<F>(&self, mut ls: Vec<F>, mut call: impl FnMut(&mut F)) -> Vec<F> {
        for f in ls.iter_mut() {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(f)));
            if let Err(e) = r {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                    .unwrap_or_else(|| "callback panicked".to_owned());
                let notify = self.with(|m| m.settings.notify_on_error);
                if notify {
                    self.notify(crate::handles::info::NotifyInfo {
                        title: Some("Callback error".to_owned()),
                        description: msg,
                        kind: Some(NotifyType::Error),
                        ..Default::default()
                    });
                }
            }
        }
        ls
    }

    // ----- settings / scheme --------------------------------------------------------------

    pub fn settings(&self) -> UiSettings {
        self.with(|m| m.settings.clone())
    }

    pub fn settings_mut(&self, f: impl FnOnce(&mut UiSettings)) {
        self.with(|m| f(&mut m.settings));
    }

    pub fn scheme(&self) -> Scheme {
        self.with(|m| m.scheme.clone())
    }

    pub fn set_scheme(&self, scheme: Scheme) {
        self.with(|m| m.scheme = scheme);
    }

    pub fn update_scheme(&self, f: impl FnOnce(&mut Scheme)) {
        self.with(|m| f(&mut m.scheme));
    }

    pub fn set_font(&self, font: epaint::FontFamily) {
        self.with(|m| m.scheme.font = font);
    }

    pub fn set_background_image(&self, image: Option<IconRef>) {
        self.with(|m| {
            m.scheme.background_image = image.clone();
            if let Some(w) = m.window {
                if let Kind::Window(wd) = &mut m.nodes[w].kind {
                    wd.background_image = image;
                }
            }
        });
    }
    pub fn set_dpi_scale(&self, percent: f32) {
        self.with(|m| m.scale = (percent / 100.0).max(0.1));
    }

    pub fn dpi_scale(&self) -> f32 {
        self.with(|m| m.scale * 100.0)
    }

    pub fn set_corner_radius(&self, radius: f32) {
        self.with(|m| {
            m.corner_radius = radius.clamp(0.0, 20.0);
            if let Some(w) = m.window {
                if let Kind::Window(wd) = &mut m.nodes[w].kind {
                    wd.corner_radius = m.corner_radius;
                }
            }
        });
    }

    pub fn corner_radius(&self) -> f32 {
        self.with(|m| m.corner_radius)
    }

    pub fn set_notify_side(&self, side: Side) {
        self.with(|m| m.settings.notify_side = side);
    }

    pub fn set_notification_type_color(&self, kind: NotifyType, color: Color32) {
        self.with(|m| {
            for (k, c) in m.settings.notification_type_colors.iter_mut() {
                if *k == kind {
                    *c = color;
                }
            }
        });
    }

    pub fn set_notification_history_limit(&self, limit: usize) {
        self.with(|m| {
            m.settings.notification_history_limit = limit.max(1);
            m.history.truncate(limit.max(1));
        });
    }

    pub fn set_notification_history_keybind(&self, key: Option<Key>) {
        self.with(|m| m.settings.notification_history_keybind = key);
    }

    /// Make a key picker's click also toggle the UI.
    pub fn set_toggle_keybind_picker(&self, picker: Option<&KeyPicker>) {
        self.with(|m| m.settings.toggle_keybind_picker = picker.map(|p| p.id));
    }

    pub fn set_cursor_icon(&self, icon: Option<IconRef>) {
        self.with(|m| {
            m.cursor_icon = icon;
            if m.cursor_icon.is_none() {
                m.cursor_icon_size = 20.0;
            }
        });
    }

    pub fn set_cursor_icon_size(&self, size: f32) {
        self.with(|m| m.cursor_icon_size = size.max(1.0));
    }

    // ----- visibility ---------------------------------------------------------------------

    /// Show/hide the whole UI. `None` flips.
    pub fn toggle(&self, value: Option<bool>) {
        self.mutate(|m| crate::window::toggle(m, value));
    }

    pub fn is_open(&self) -> bool {
        self.with(|m| m.open)
    }

    /// True while the UI is open and `unlock_mouse_while_open` is set (hosts may unlock the game cursor).
    pub fn wants_mouse(&self) -> bool {
        self.with(|m| m.open && m.settings.unlock_mouse_while_open)
    }

    // ----- registries ---------------------------------------------------------------------

    pub fn toggles(&self) -> HashMap<String, Toggle> {
        self.with(|m| m.toggles.iter().map(|(k, v)| (k.clone(), Toggle { ui: self.clone(), id: *v })).collect())
    }

    pub fn toggle_by(&self, idx: &str) -> Option<Toggle> {
        self.with(|m| m.toggles.get(idx).map(|id| Toggle { ui: self.clone(), id: *id }))
    }

    pub fn options(&self) -> HashMap<String, OptionHandle> {
        self.with(|m| {
            m.options.iter().filter_map(|(k, v)| OptionHandle::from_node(self, m, *v).map(|h| (k.clone(), h))).collect()
        })
    }

    pub fn option(&self, idx: &str) -> Option<OptionHandle> {
        self.with(|m| m.options.get(idx).and_then(|id| OptionHandle::from_node(self, m, *id)))
    }

    pub fn labels(&self) -> Vec<Label> {
        self.with(|m| m.labels.iter().map(|id| Label { ui: self.clone(), id: *id }).collect())
    }

    pub fn buttons(&self) -> Vec<Button> {
        self.with(|m| m.buttons.iter().map(|id| Button { ui: self.clone(), id: *id }).collect())
    }

    // ----- lifecycle ----------------------------------------------------------------------

    pub fn on_unload(&self, f: impl FnOnce() + Send + 'static) {
        self.with(|m| m.unload_callbacks.push(Box::new(f)));
    }

    /// Run unload callbacks, then drop everything.
    pub fn unload(&self) {
        let cbs = self.with(|m| {
            m.unloaded = true;
            std::mem::take(&mut m.unload_callbacks)
        });
        for cb in cbs {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(cb));
        }
        self.with(|m| {
            let fresh = Model::new();
            *m = fresh;
            m.unloaded = true;
        });
    }

    pub fn is_unloaded(&self) -> bool {
        self.with(|m| m.unloaded)
    }

    // ----- search -------------------------------------------------------------------------
    pub fn update_search(&self, text: &str) {
        self.mutate(|m| {
            if let Some(w) = m.window {
                if let Kind::Window(wd) = &mut m.nodes[w].kind {
                    wd.search_text = text.to_owned();
                }
            }
            crate::overlays::search::update_search(m, text);
        });
    }

    // ----- frame --------------------------------------------------------------------------

    /// Icons the last frame needed (backends may pre-resolve them outside the font lock).
    pub fn icons_needed(&self) -> Vec<(IconRef, u32)> {
        self.with(|m| m.icons_needed.clone())
    }

    /// Run one frame: layout, interaction and painting in a single pass.
    pub fn frame(&self, mut fi: FrameInput<'_>) -> FrameOutput {
        let mut out = FrameOutput::default();
        {
            let mut m = self.model.lock();
            if m.unloaded {
                return out;
            }
            m.time = fi.time;
            m.screen = fi.screen;
            m.ppp = fi.pixels_per_point;
            let scheme = m.scheme.clone();
            let radius = m.corner_radius;
            let scale = m.scale;
            let mut cx = Cx {
                fi: &mut fi,
                out: &mut out,
                layer: Layer::Window,
                clip: m.screen,
                blocked: false,
                m: M { s: scale, ppp: m.ppp },
                sch: scheme,
                radius,
                time: m.time,
                dt: m.time as f32 * 0.0 + 0.0,
                fade: 1.0,
                animating: false,
                tooltip: None,
                key_tab: false,
                content_covered: false,
            };
            cx.dt = cx.fi.dt;
            crate::window::frame(&mut m, &mut cx);
            if cx.animating {
                cx.out.repaint_after = Some(1.0 / 120.0);
            }
            m.first_frame_done = true;
        }
        self.flush_pending();
        out
    }
}
