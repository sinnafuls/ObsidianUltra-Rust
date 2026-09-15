//! Retained object model. Every window part and element is a `Node` in one arena.

use std::collections::HashMap;

use epaint::{Color32, Pos2, Rect, Vec2};

use crate::color::Rgba;
use crate::handles::info::*;
use crate::layout::Scroll;
use crate::tween::Anim;
use crate::types::*;
use crate::input::Modifier;

slotmap::new_key_type! {
    /// Identity of a retained node.
    pub struct NodeId;
}

pub type BoolFn = Box<dyn FnMut(bool) + Send>;
pub type F64Fn = Box<dyn FnMut(f64) + Send>;
pub type StrFn = Box<dyn FnMut(&str) + Send>;
pub type DropFn = Box<dyn FnMut(&DropdownValue) + Send>;
pub type ListFn = Box<dyn FnMut(&[String]) + Send>;
pub type KeyFn = Box<dyn FnMut(&KeyBind) + Send>;
pub type ColorFn = Box<dyn FnMut(Rgba) + Send>;
pub type UnitFn = Box<dyn FnMut() + Send>;
pub type DialogFn = Box<dyn FnMut(&crate::handles::Dialog) + Send>;
pub type LoadingFn = Box<dyn FnMut(&crate::handles::Loading) + Send>;
pub type VerifyFn = Box<dyn Fn(&str) -> bool + Send + Sync>;
pub type FormatF64Fn = Box<dyn Fn(f64) -> String + Send + Sync>;
pub type FormatStrFn = Box<dyn Fn(&str) -> String + Send + Sync>;
pub type GetterFn = Box<dyn Fn() -> String + Send + Sync>;
pub type CustomDrawFn = Box<dyn FnMut(&mut crate::handles::CustomCtx<'_>) + Send>;

/// One retained object.
pub struct Node {
    pub kind: Kind,
    pub parent: Option<NodeId>,
    /// Ordered children for containers (elements), tabs (boxes) etc.
    pub children: Vec<NodeId>,
    /// Registry key (`""` when unregistered).
    pub idx: String,
    /// User-controlled visibility.
    pub visible: bool,
    /// Search filter result; `true` when no search is active.
    pub search_visible: bool,
    pub destroyed: bool,
    /// Screen rect from the last frame (hit-testing, overlay blocking).
    pub rect: Rect,
    /// Generic hover animation (0 = idle, 1 = hovered).
    pub hover: Anim,
}

impl Node {
    pub fn new(kind: Kind, parent: Option<NodeId>) -> Self {
        Self {
            kind,
            parent,
            children: Vec::new(),
            idx: String::new(),
            visible: true,
            search_visible: true,
            destroyed: false,
            rect: Rect::NOTHING,
            hover: Anim::new(0.0),
        }
    }

    pub fn shown(&self) -> bool {
        self.visible && self.search_visible && !self.destroyed
    }
}

/// Node payloads.
pub enum Kind {
    Window(Box<WindowData>),
    Tab(Box<TabData>),
    SubTab(Box<SubTabData>),
    Tabbox(TabboxData),
    TabboxTab(TabboxTabData),
    Groupbox(GroupboxData),
    DepBox(DepData),
    DepGroupbox(DepData),
    Dialog(DialogData),
    KeyTab(KeyTabData),
    Loading(Box<LoadingData>),
    LoadingSidebar,
    DraggableMenu(DraggableMenuData),
    DraggableLabel(DraggableLabelData),
    DraggableButton(DraggableButtonData),
    DraggableImageButton(DraggableImageButtonData),
    Watermark(WatermarkData),
    Notification(NotifyData),
    MinimizedLabel(MinimizedLabelData),
    Divider(DividerData),
    Label(LabelData),
    Button(ButtonData),
    Toggle(ToggleData),
    Input(InputData),
    Slider(SliderData),
    Dropdown(Box<DropdownData>),
    Priority(Box<PriorityData>),
    Image(ImageData),
    ProfileCard(ProfileData),
    Custom(CustomData),
    KeyPicker(Box<KeyPickerData>),
    ColorPicker(Box<ColorPickerData>),
    KeyBox(KeyBoxData),
}

impl Kind {
    /// `ElementInfo.Type` for search / dependency logic.
    pub fn type_name(&self) -> &'static str {
        match self {
            Kind::Window(_) => "Window",
            Kind::Tab(_) => "Tab",
            Kind::SubTab(_) => "SubTab",
            Kind::Tabbox(_) => "Tabbox",
            Kind::TabboxTab(_) => "TabboxTab",
            Kind::Groupbox(_) => "Groupbox",
            Kind::DepBox(_) => "DependencyBox",
            Kind::DepGroupbox(_) => "DependencyGroupbox",
            Kind::Dialog(_) => "Dialog",
            Kind::KeyTab(_) => "KeyTab",
            Kind::Loading(_) => "Loading",
            Kind::LoadingSidebar => "LoadingSidebar",
            Kind::DraggableMenu(_) => "DraggableMenu",
            Kind::DraggableLabel(_) => "DraggableLabel",
            Kind::DraggableButton(_) => "DraggableButton",
            Kind::DraggableImageButton(_) => "DraggableImageButton",
            Kind::Watermark(_) => "Watermark",
            Kind::Notification(_) => "Notification",
            Kind::MinimizedLabel(_) => "MinimizedLabel",
            Kind::Divider(_) => "Divider",
            Kind::Label(_) => "Label",
            Kind::Button(b) => {
                if b.is_sub { "SubButton" } else { "Button" }
            }
            Kind::Toggle(_) => "Toggle",
            Kind::Input(_) => "Input",
            Kind::Slider(_) => "Slider",
            Kind::Dropdown(_) => "Dropdown",
            Kind::Priority(_) => "PriorityDropdown",
            Kind::Image(_) => "Image",
            Kind::ProfileCard(_) => "PlayerInfo",
            Kind::Custom(_) => "UIPassthrough",
            Kind::KeyPicker(_) => "KeyPicker",
            Kind::ColorPicker(_) => "ColorPicker",
            Kind::KeyBox(_) => "KeyBox",
        }
    }

}

// ----- common element bits ---------------------------------------------------------------

/// Tooltip texts shared by interactive elements.
#[derive(Clone, Debug, Default)]
pub struct Tips {
    pub tooltip: Option<String>,
    pub disabled_tooltip: Option<String>,
}

impl Tips {
    pub fn text(&self, disabled: bool) -> Option<&str> {
        if disabled { self.disabled_tooltip.as_deref() } else { self.tooltip.as_deref() }
    }
}

/// State of a context menu owned by an element.
#[derive(Clone, Copy, Debug)]
pub struct MenuState {
    pub open: bool,
    /// 0 -> 1 open progress (height animation).
    pub anim: Anim,
    pub rect: Rect,
    pub holder: Rect,
    /// Frame time the menu was toggled, used to ignore the opening click.
    pub opened_at: f64,
}

impl Default for MenuState {
    fn default() -> Self {
        Self::NONE
    }
}

impl MenuState {
    pub const NONE: MenuState = MenuState { open: false, anim: Anim::new(0.0), rect: Rect::NOTHING, holder: Rect::NOTHING, opened_at: 0.0 };
}

/// Pop-out state of a groupbox/tabbox.
#[derive(Clone, Copy, Debug)]
pub struct PopOut {
    pub enabled: bool,
    pub popped: bool,
    pub pos: Pos2,
    pub width: f32,
    pub z: u32,
    pub hold_since: Option<f64>,
    pub press_pos: Pos2,
    pub drag_start: Option<(Pos2, Pos2)>,
    pub did_move: bool,
    /// Placeholder rect (last frame) while popped.
    pub placeholder: Rect,
    /// Header rect (last frame) used as the drag source.
    pub header: Rect,
}

impl PopOut {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            popped: false,
            pos: Pos2::ZERO,
            width: 200.0,
            z: 0,
            hold_since: None,
            press_pos: Pos2::ZERO,
            drag_start: None,
            did_move: false,
            placeholder: Rect::NOTHING,
            header: Rect::NOTHING,
        }
    }
}

// ----- window / tabs ---------------------------------------------------------------------

pub struct WindowData {
    pub info: WindowInfo,
    /// Position/size finalized against the real screen on the first frame.
    pub placed: bool,
    pub title: String,
    pub pos: Pos2,
    pub size: Vec2,
    pub min_size: Vec2,
    pub sidebar_width: f32,
    pub last_expanded_width: f32,
    pub compact: bool,
    pub minimized: bool,
    pub mini_pos: Pos2,
    pub mini_subtitle: String,
    pub mini_subtitle_explicit: bool,
    pub mini_labels: Vec<NodeId>,
    pub tabs: Vec<NodeId>,
    pub active_tab: Option<NodeId>,
    pub footer: Footer,
    /// (segment index, time) of the last successful copy.
    pub footer_copied: Option<(usize, f64)>,
    pub glow: GlowConfig,
    pub snapping: bool,
    pub snap_distance: f32,
    pub snap_margin: f32,
    pub search_text: String,
    /// Text sent in the previous frame's field request (stale-response guard).
    pub search_sent: String,
    pub search_focus: crate::frame::FocusRequest,
    pub search_focus_anim: Anim,
    pub dialogs: Vec<NodeId>,
    pub always_on_top: bool,
    pub background_image: Option<IconRef>,
    pub corner_radius: f32,
    pub tab_transition_time: f32,
    pub tab_swipe_offset: f32,
    pub tab_swipe_from: SwipeFrom,
    /// Sidebar grabber hover animation.
    pub grabber_hover: Anim,
    pub minimize_hover: Anim,
    pub bell_hover: Anim,
    pub mini_bell_hover: Anim,
    /// Snap guide lines (screen x / y) shown during a drag.
    pub snap_guide_x: Option<f32>,
    pub snap_guide_y: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub struct GlowConfig {
    pub enabled: bool,
    pub transparency: f32,
    pub radius: f32,
    pub color: Option<Color32>,
}

impl Default for GlowConfig {
    fn default() -> Self {
        Self { enabled: false, transparency: 0.35, radius: 16.0, color: None }
    }
}

#[derive(Clone, Debug)]
pub struct WarningBox {
    pub is_normal: bool,
    pub lock_size: bool,
    pub visible: bool,
    pub title: String,
    pub text: String,
    pub scroll: Scroll,
}

impl Default for WarningBox {
    fn default() -> Self {
        Self { is_normal: false, lock_size: false, visible: false, title: "WARNING".to_owned(), text: String::new(), scroll: Scroll::default() }
    }
}

pub struct TabData {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<IconRef>,
    pub order: i32,
    pub layout: Layout,
    pub tips: Tips,
    /// Groupboxes, tabboxes and dependency groupboxes in insertion order (each knows its side).
    pub boxes: Vec<NodeId>,
    pub sub_tabs: Vec<NodeId>,
    pub active_sub_tab: Option<NodeId>,
    pub sidebar_expanded: bool,
    pub sidebar_list: Anim,
    pub chevron: Anim,
    pub warning: WarningBox,
    pub banners: Vec<NodeId>,
    pub sub_tab_align: Align,
    pub underline_x: Anim,
    pub underline_w: Anim,
    pub underline_visible: bool,
    pub scroll: [Scroll; 2],
    /// Tab canvas transition progress (0 = hidden, 1 = shown).
    pub canvas: Anim,
    pub active: Anim,
}

pub struct SubTabData {
    pub name: String,
    pub icon: Option<IconRef>,
    pub boxes: Vec<NodeId>,
    pub scroll: [Scroll; 2],
    pub canvas: Anim,
    pub active: Anim,
    pub chip: Anim,
    pub scale: Anim,
    pub entry_active: Anim,
    pub entry_hover: Anim,
}

pub struct TabboxData {
    pub side: Side,
    pub name: Option<String>,
    pub tabs: Vec<NodeId>,
    pub active: Option<NodeId>,
    pub in_groupbox: bool,
    pub underline_x: Anim,
    pub underline_w: Anim,
    pub underline_visible: bool,
    pub pop: PopOut,
    pub height: f32,
}

pub struct TabboxTabData {
    pub name: Option<String>,
    pub icon: Option<IconRef>,
    pub slide: Anim,
    pub active: Anim,
}

pub struct GroupboxData {
    pub side: Side,
    pub name: String,
    pub icon: Option<IconRef>,
    pub description: Option<String>,
    pub collapsed: bool,
    pub disable_collapsing: bool,
    pub chevron: Anim,
    pub height: Anim,
    pub pop: PopOut,
    pub content_height: f32,
}

/// Dependency box / dependency groupbox.
pub struct DepData {
    pub side: Side,
    pub deps: Vec<Dependency>,
    pub satisfied: bool,
    pub height: f32,
}

/// A dependency condition.
pub enum Dependency {
    Toggle(NodeId, bool),
    Dropdown(NodeId, String),
}

pub struct DialogButton {
    pub key: String,
    pub title: String,
    pub variant: ButtonVariant,
    pub order: i32,
    pub disabled: bool,
    pub wait_until: Option<f64>,
    pub wait_time: f32,
    pub callback: Vec<DialogFn>,
    pub hover: Anim,
    pub rect: Rect,
}

pub struct DialogData {
    pub title: String,
    pub description: String,
    pub icon: Option<IconRef>,
    pub title_color: Option<Color32>,
    pub description_color: Option<Color32>,
    pub auto_dismiss: bool,
    pub outside_click_dismiss: bool,
    pub buttons: Vec<DialogButton>,
    pub open: Anim,
    pub closing: bool,
    pub close_at: f64,
}

pub struct KeyTabData {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<IconRef>,
    pub order: i32,
    pub scroll: Scroll,
    pub canvas: Anim,
    pub active: Anim,
}

pub struct KeyBoxData {
    pub text: String,
    pub callback: Vec<StrFn>,
    pub focus: Anim,
}

pub struct LoadingButton {
    pub title: String,
    pub variant: ButtonVariant,
    pub callback: Vec<LoadingFn>,
    pub hover: Anim,
    pub rect: Rect,
}

pub struct LoadingData {
    pub info: LoadingInfo,
    pub message: String,
    pub description: String,
    pub current_step: u32,
    pub total_steps: u32,
    pub show_sidebar: bool,
    pub auto_resize_height: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub base_height: f32,
    pub content_width: f32,
    pub sidebar_width: f32,
    pub is_error: bool,
    pub error_message: String,
    pub error_buttons: Vec<LoadingButton>,
    pub sidebar: NodeId,
    pub pos: Pos2,
    pub placed: bool,
    pub w: Anim,
    pub h: Anim,
    pub step: Anim,
    pub spinner_start: f64,
    pub loading_icon: IconRef,
    pub loading_icon_color: Option<Color32>,
    pub spin_time: f32,
    pub hid_window: bool,
}

// ----- floats ----------------------------------------------------------------------------

pub struct DraggableMenuData {
    pub name: String,
    pub pos: Pos2,
    pub placed: bool,
    pub z: u32,
    /// The built-in keybind frame anchors left-center and auto-hides when empty.
    pub is_keybind_frame: bool,
}

pub struct DraggableLabelData {
    pub text: String,
    pub icon: Option<IconRef>,
    pub icon_position: IconPosition,
    pub pos: Pos2,
    pub placed: bool,
    pub z: u32,
}

pub struct DraggableButtonData {
    pub text: String,
    pub callback: Vec<UnitFn>,
    pub exclude_scaling: bool,
    pub exclude_dragging: bool,
    pub pos: Pos2,
    pub placed: bool,
    pub z: u32,
    pub press_since: Option<f64>,
}

pub struct DraggableImageButtonData {
    pub icon: IconRef,
    pub icon_size: f32,
    pub callback: Vec<UnitFn>,
    pub exclude_scaling: bool,
    pub exclude_dragging: bool,
    pub pos: Pos2,
    pub placed: bool,
    pub z: u32,
    pub press_since: Option<f64>,
}

pub struct WatermarkData {
    pub segments: Vec<WatermarkSegment>,
    pub texts: Vec<String>,
    pub refresh_rate: f32,
    pub last_refresh: f64,
    pub pos: Pos2,
    pub placed: bool,
    pub z: u32,
}

pub struct MinimizedLabelData {
    pub text: String,
}

// ----- notifications --------------------------------------------------------------------

pub struct NotifyData {
    pub title: Option<String>,
    pub title_color: Option<Color32>,
    pub description: String,
    pub description_color: Option<Color32>,
    pub time: f32,
    pub steps: Option<u32>,
    pub step: u32,
    pub persist: bool,
    pub icon: Option<IconRef>,
    pub big_icon: Option<IconRef>,
    pub icon_color: Option<Color32>,
    pub kind: Option<NotifyType>,
    pub created: f64,
    /// 0 = off-screen, 1 = in place.
    pub slide: Anim,
    pub y: Anim,
    pub y_initialized: bool,
    pub destroying: bool,
    pub destroy_at: f64,
    pub height: f32,
    pub width: f32,
}

// ----- elements --------------------------------------------------------------------------

pub struct DividerData {
    pub text: Option<String>,
    pub margin_top: f32,
    pub margin_bottom: f32,
}

pub struct LabelData {
    pub text: String,
    pub does_wrap: bool,
    pub size: f32,
    pub addons: Vec<NodeId>,
    pub height: f32,
}

pub struct ButtonData {
    pub text: String,
    pub func: Vec<UnitFn>,
    pub double_click: bool,
    pub tips: Tips,
    pub risky: bool,
    pub disabled: bool,
    pub is_sub: bool,
    pub sub: Option<NodeId>,
    pub confirm_until: Option<f64>,
    pub addons: Vec<NodeId>,
}

pub struct ToggleData {
    pub text: String,
    pub value: bool,
    pub variant: ToggleVariant,
    pub listeners: Vec<BoolFn>,
    pub tips: Tips,
    pub risky: bool,
    pub disabled: bool,
    pub addons: Vec<NodeId>,
    pub ball: Anim,
    pub label: Anim,
    pub default: bool,
}

pub struct InputData {
    pub text: String,
    pub value: String,
    pub finished: bool,
    pub numeric: bool,
    pub clear_text_on_focus: bool,
    pub clear_text_on_blur: bool,
    pub placeholder: String,
    pub allow_empty: bool,
    pub empty_reset: String,
    pub max_length: Option<usize>,
    pub verify: Option<VerifyFn>,
    pub listeners: Vec<StrFn>,
    pub tips: Tips,
    pub disabled: bool,
    pub focus: Anim,
    pub default: String,
    /// Text currently shown in the hosted field (may differ from `value` while editing).
    pub editing: String,
}

pub struct SliderData {
    pub text: String,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub rounding: u8,
    pub prefix: String,
    pub suffix: String,
    pub compact: bool,
    pub hide_max: bool,
    pub format: Option<FormatF64Fn>,
    pub allow_right_click_input: bool,
    pub listeners: Vec<F64Fn>,
    pub tips: Tips,
    pub disabled: bool,
    pub ball: Anim,
    pub editing: Option<String>,
    pub edit_focus: crate::frame::FocusRequest,
    pub default: f64,
}

pub struct DropdownData {
    pub text: Option<String>,
    pub values: Vec<DropdownEntry>,
    pub disabled_values: Vec<String>,
    pub value_images: HashMap<String, IconRef>,
    pub multi: bool,
    pub drag_select: bool,
    pub max_visible: usize,
    pub select_all_buttons: bool,
    pub expandable: bool,
    pub expand_columns: usize,
    pub searchable: bool,
    pub format_list: Option<FormatStrFn>,
    pub format_display: Option<FormatStrFn>,
    pub allow_null: bool,
    pub value: DropdownValue,
    pub listeners: Vec<DropFn>,
    pub tips: Tips,
    pub disabled: bool,
    pub menu: MenuState,
    pub scroll: Scroll,
    pub search: String,
    pub search_focus: crate::frame::FocusRequest,
    pub expanded: bool,
    pub expand: Anim,
    pub expand_search: String,
    pub expand_scroll: Scroll,
    pub drag: Option<DragSelect>,
    pub default: DropdownValue,
    pub arrow: Anim,
    pub focus: Anim,
    /// Filtered entry indices (into `values`) for the open list; rebuilt when dirty.
    pub filtered: Vec<usize>,
    pub filter_dirty: bool,
    pub row_hover: Vec<Anim>,
}

pub struct DragSelect {
    pub start: usize,
    pub last_range: Option<(usize, usize)>,
    pub initial: HashMap<String, bool>,
    pub active_count: usize,
}

pub struct PriorityData {
    pub text: Option<String>,
    pub values: Vec<String>,
    pub value: Vec<String>,
    pub max_visible: usize,
    pub searchable: bool,
    pub expandable: bool,
    pub format: Option<FormatStrFn>,
    pub listeners: Vec<ListFn>,
    pub tips: Tips,
    pub disabled: bool,
    pub menu: MenuState,
    pub scroll: Scroll,
    pub search: String,
    pub search_focus: crate::frame::FocusRequest,
    pub expanded: bool,
    pub expand: Anim,
    pub expand_search: String,
    pub expand_scroll: Scroll,
    pub drag: Option<PriorityDrag>,
    pub row_y: Vec<Anim>,
    pub expand_row_y: Vec<Anim>,
    pub default: Vec<String>,
    pub arrow: Anim,
    pub focus: Anim,
}

pub struct PriorityDrag {
    pub value: String,
    pub grab_offset: f32,
    pub expanded: bool,
    pub pointer_y: f32,
}

pub struct ImageData {
    pub image: IconRef,
    pub transparency: f32,
    pub background_transparency: f32,
    pub color: Color32,
    pub uv: Option<Rect>,
    pub scale_type: ScaleType,
    pub height: f32,
}

pub struct ProfileData {
    pub avatar: Option<IconRef>,
    pub name: String,
    pub title: String,
    pub description: Description,
    pub height: Option<f32>,
    pub compact: bool,
}

pub struct CustomData {
    pub height: f32,
    pub draw: CustomDrawFn,
}

/// Key picker picking-flow state.
#[derive(Clone, Debug, Default)]
pub struct Picking {
    pub active: bool,
    pub modifiers: Vec<Modifier>,
    pub current_modifier: Option<Modifier>,
    /// The picked key is held until released; picking ends on release.
    pub waiting_release: Option<KeyValue>,
}

pub struct KeyPickerData {
    pub text: String,
    pub value: KeyBind,
    pub modes: Vec<KeyMode>,
    pub sync_toggle_state: bool,
    pub no_ui: bool,
    pub wait_for_callback: bool,
    pub blacklisted: Vec<KeyValue>,
    pub blacklisted_modifiers: Vec<Modifier>,
    pub whitelisted: Vec<KeyValue>,
    pub whitelisted_modifiers: Vec<Modifier>,
    pub toggled: bool,
    pub picking: Picking,
    pub changed: Vec<KeyFn>,
    pub callback: Vec<BoolFn>,
    pub clicked: Vec<BoolFn>,
    pub menu: MenuState,
    pub host: NodeId,
    pub host_is_button: bool,
    pub default: KeyBind,
    pub marquee_start: f64,
    pub marquee_text: String,
    pub hold_active: bool,
}

pub struct ColorPickerData {
    pub hue: f32,
    pub sat: f32,
    pub vib: f32,
    pub transparency: f32,
    pub has_alpha: bool,
    pub title: Option<String>,
    pub resizable: bool,
    pub listeners: Vec<ColorFn>,
    pub menu: MenuState,
    pub ctx_menu: MenuState,
    pub map_w: f32,
    pub map_h: f32,
    pub default: Rgba,
    /// (button index, text, until) feedback for copy/paste buttons.
    pub feedback: Option<(u8, String, f64)>,
    pub hex_focus: Anim,
    pub rgb_focus: Anim,
    pub resize_start: Option<(Pos2, f32, f32)>,
}

impl ColorPickerData {
    pub fn value(&self) -> Rgba {
        Rgba { color: crate::color::from_hsv(self.hue, self.sat, self.vib), transparency: if self.has_alpha { self.transparency } else { 0.0 } }
    }
}

/// Mutable capture of the pointer by one interaction (drag/resizes/slider...).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capture {
    pub node: NodeId,
    pub sub: u8,
    pub start: Pos2,
    pub data: [f32; 4],
    pub since: f64,
}

pub mod cap {
    pub const WINDOW_DRAG: u8 = 1;
    pub const WINDOW_RESIZE: u8 = 2;
    pub const SIDEBAR: u8 = 3;
    pub const MINI_DRAG: u8 = 4;
    pub const SLIDER: u8 = 5;
    pub const SV: u8 = 6;
    pub const HUE: u8 = 7;
    pub const ALPHA: u8 = 8;
    pub const CP_RESIZE: u8 = 9;
    pub const POPOUT: u8 = 10;
    pub const FLOAT_DRAG: u8 = 11;
    pub const DRAG_SELECT: u8 = 12;
    pub const PRIORITY: u8 = 13;
    pub const LOADING_DRAG: u8 = 14;
    pub const HISTORY_DRAG: u8 = 15;
}
