//! Option structs (`Info` tables) for every constructor. All implement `Default`
//! with the original library's template values.

use epaint::{Color32, FontFamily, Pos2, Rect, Vec2};

use crate::color::Rgba;
use crate::input::{Key, Modifier};
use crate::node::*;
use crate::types::*;

/// Animation feature flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Animations {
    pub toggle_window: bool,
    pub tab_switch: bool,
    pub groupbox: bool,
    pub dropdown: bool,
    pub key_picker: bool,
    pub sub_tab_underline: bool,
    pub sidebar_sub_tabs: bool,
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            toggle_window: false,
            tab_switch: false,
            groupbox: false,
            dropdown: false,
            key_picker: false,
            sub_tab_underline: true,
            sidebar_sub_tabs: true,
        }
    }
}

impl Animations {
    /// Everything on.
    pub const ALL: Animations = Animations {
        toggle_window: true,
        tab_switch: true,
        groupbox: true,
        dropdown: true,
        key_picker: true,
        sub_tab_underline: true,
        sidebar_sub_tabs: true,
    };
}

pub struct WindowInfo {
    pub title: String,
    pub footer: Footer,
    pub copyable_footer: bool,
    pub icon: Option<IconRef>,
    pub icon_size: f32,
    pub position: Pos2,
    pub size: Vec2,
    pub auto_show: bool,
    pub center: bool,
    pub resizable: bool,
    pub snapping: bool,
    pub snap_distance: f32,
    pub snap_margin: f32,
    pub searchbar_width_frac: f32,
    pub disable_search: bool,
    pub global_search: bool,
    pub fuzzy_search: bool,
    pub search_values: bool,
    pub search_keybind: Key,
    pub disable_search_keybind: bool,
    pub minimizable: bool,
    pub minimize_keybind: Option<Key>,
    pub minimized_width: f32,
    pub minimized_subtitle: String,
    pub auto_minimize: bool,
    pub corner_radius: f32,
    pub notify_side: Side,
    pub disable_notification_bell: bool,
    pub show_custom_cursor: bool,
    pub glow: bool,
    pub font: FontFamily,
    pub toggle_keybind: Option<Key>,
    pub unlock_mouse_while_open: bool,
    pub enable_sidebar_resize: bool,
    pub enable_compacting: bool,
    pub disable_compacting_snap: bool,
    pub sidebar_compacted: bool,
    pub min_container_width: f32,
    pub min_sidebar_width: f32,
    pub sidebar_compact_width: f32,
    pub sidebar_collapse_threshold: f32,
    pub compact_width_activation: f32,
    pub background_image: Option<IconRef>,
    pub animations: Animations,
    pub tab_transition_time: f32,
    pub tab_swipe_offset: f32,
    pub tab_swipe_from: SwipeFrom,
    pub always_on_top: bool,
}

impl Default for WindowInfo {
    fn default() -> Self {
        Self {
            title: "No Title".to_owned(),
            footer: Footer::default(),
            copyable_footer: true,
            icon: None,
            icon_size: 30.0,
            position: Pos2::new(6.0, 6.0),
            size: Vec2::new(720.0, 600.0),
            auto_show: true,
            center: true,
            resizable: true,
            snapping: false,
            snap_distance: 28.0,
            snap_margin: 8.0,
            searchbar_width_frac: 0.35,
            disable_search: false,
            global_search: false,
            fuzzy_search: true,
            search_values: true,
            search_keybind: Key::F,
            disable_search_keybind: false,
            minimizable: true,
            minimize_keybind: None,
            minimized_width: 300.0,
            minimized_subtitle: String::new(),
            auto_minimize: false,
            corner_radius: 4.0,
            notify_side: Side::Right,
            disable_notification_bell: false,
            show_custom_cursor: true,
            glow: false,
            font: FontFamily::Monospace,
            toggle_keybind: Some(Key::RightControl),
            unlock_mouse_while_open: true,
            enable_sidebar_resize: true,
            enable_compacting: true,
            disable_compacting_snap: false,
            sidebar_compacted: false,
            min_container_width: 256.0,
            min_sidebar_width: 128.0,
            sidebar_compact_width: 48.0,
            sidebar_collapse_threshold: 0.5,
            compact_width_activation: 128.0,
            background_image: None,
            animations: Animations::default(),
            tab_transition_time: 0.22,
            tab_swipe_offset: 26.0,
            tab_swipe_from: SwipeFrom::Bottom,
            always_on_top: false,
        }
    }
}

#[derive(Default)]
pub struct TabInfo {
    pub name: String,
    pub icon: Option<IconRef>,
    pub description: Option<String>,
    pub order: Option<i32>,
    pub layout: Layout,
    pub tooltip: Option<String>,
    pub disabled_tooltip: Option<String>,
}

impl TabInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }
    pub fn icon(mut self, icon: impl Into<IconRef>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    pub fn single_column(mut self) -> Self {
        self.layout = Layout::Single;
        self
    }
}

pub struct KeyTabInfo {
    pub name: String,
    pub icon: Option<IconRef>,
    pub description: Option<String>,
    pub order: Option<i32>,
}

impl Default for KeyTabInfo {
    fn default() -> Self {
        Self { name: "Tab".to_owned(), icon: Some(IconRef::Lucide("key".to_owned())), description: None, order: None }
    }
}

pub struct SubTabInfo {
    pub name: String,
    pub icon: Option<IconRef>,
}

impl Default for SubTabInfo {
    fn default() -> Self {
        Self { name: "SubTab".to_owned(), icon: None }
    }
}

impl SubTabInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), icon: None }
    }
}

pub struct GroupboxInfo {
    pub side: Side,
    pub name: String,
    pub icon: Option<IconRef>,
    pub description: Option<String>,
    pub visible: bool,
    pub collapsed: bool,
    pub disable_collapsing: bool,
    pub pop_out: bool,
}

impl Default for GroupboxInfo {
    fn default() -> Self {
        Self {
            side: Side::Left,
            name: "Groupbox".to_owned(),
            icon: None,
            description: None,
            visible: true,
            collapsed: false,
            disable_collapsing: false,
            pop_out: true,
        }
    }
}

impl GroupboxInfo {
    pub fn new(name: impl Into<String>, side: Side) -> Self {
        Self { name: name.into(), side, ..Default::default() }
    }
    pub fn left(name: impl Into<String>) -> Self {
        Self::new(name, Side::Left)
    }
    pub fn right(name: impl Into<String>) -> Self {
        Self::new(name, Side::Right)
    }
    pub fn icon(mut self, icon: impl Into<IconRef>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
}

pub struct TabboxInfo {
    pub side: Side,
    pub name: Option<String>,
    pub pop_out: bool,
}

impl Default for TabboxInfo {
    fn default() -> Self {
        Self { side: Side::Left, name: None, pop_out: true }
    }
}

impl TabboxInfo {
    pub fn new(side: Side) -> Self {
        Self { side, ..Default::default() }
    }
}

#[derive(Default)]
pub struct WarningBoxInfo {
    pub is_normal: Option<bool>,
    pub lock_size: Option<bool>,
    pub visible: Option<bool>,
    pub title: Option<String>,
    pub text: Option<String>,
}

pub struct DialogInfo {
    pub title: String,
    pub description: String,
    pub auto_dismiss: bool,
    pub outside_click_dismiss: bool,
    pub icon: Option<IconRef>,
    pub title_color: Option<Color32>,
    pub description_color: Option<Color32>,
    pub footer_buttons: Vec<(String, FooterButtonInfo)>,
}

impl Default for DialogInfo {
    fn default() -> Self {
        Self {
            title: "Dialog".to_owned(),
            description: "Description".to_owned(),
            auto_dismiss: true,
            outside_click_dismiss: true,
            icon: None,
            title_color: None,
            description_color: None,
            footer_buttons: Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct FooterButtonInfo {
    pub title: Option<String>,
    pub callback: Option<DialogFn>,
    pub variant: ButtonVariant,
    pub order: i32,
    pub wait_time: Option<f32>,
}

#[derive(Default)]
pub struct DividerInfo {
    pub text: Option<String>,
    pub margin_top: f32,
    pub margin_bottom: f32,
}

impl DividerInfo {
    pub fn text(t: impl Into<String>) -> Self {
        Self { text: Some(t.into()), ..Default::default() }
    }
}

pub struct LabelInfo {
    pub text: String,
    pub does_wrap: bool,
    pub size: f32,
    pub visible: bool,
}

impl Default for LabelInfo {
    fn default() -> Self {
        Self { text: String::new(), does_wrap: false, size: 14.0, visible: true }
    }
}

impl LabelInfo {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Default::default() }
    }
    pub fn wrap(mut self) -> Self {
        self.does_wrap = true;
        self
    }
}

pub struct ButtonInfo {
    pub text: String,
    pub callback: Option<UnitFn>,
    pub double_click: bool,
    pub tooltip: Option<String>,
    pub disabled_tooltip: Option<String>,
    pub risky: bool,
    pub disabled: bool,
    pub visible: bool,
}

impl Default for ButtonInfo {
    fn default() -> Self {
        Self {
            text: String::new(),
            callback: None,
            double_click: false,
            tooltip: None,
            disabled_tooltip: None,
            risky: false,
            disabled: false,
            visible: true,
        }
    }
}

impl ButtonInfo {
    pub fn new(text: impl Into<String>, f: impl FnMut() + Send + 'static) -> Self {
        Self { text: text.into(), callback: Some(Box::new(f)), ..Default::default() }
    }
}

pub struct ToggleInfo {
    pub text: String,
    pub default: bool,
    pub callback: Option<BoolFn>,
    pub risky: bool,
    pub disabled: bool,
    pub visible: bool,
    pub tooltip: Option<String>,
    pub disabled_tooltip: Option<String>,
}

impl Default for ToggleInfo {
    fn default() -> Self {
        Self {
            text: "Toggle".to_owned(),
            default: false,
            callback: None,
            risky: false,
            disabled: false,
            visible: true,
            tooltip: None,
            disabled_tooltip: None,
        }
    }
}

impl ToggleInfo {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Default::default() }
    }
    pub fn default_value(mut self, v: bool) -> Self {
        self.default = v;
        self
    }
    pub fn tooltip(mut self, t: impl Into<String>) -> Self {
        self.tooltip = Some(t.into());
        self
    }
    pub fn on_changed(mut self, f: impl FnMut(bool) + Send + 'static) -> Self {
        self.callback = Some(Box::new(f));
        self
    }
}

pub struct InputInfo {
    pub text: String,
    pub default: String,
    pub finished: bool,
    pub numeric: bool,
    pub clear_text_on_focus: bool,
    pub clear_text_on_blur: bool,
    pub placeholder: String,
    pub allow_empty: bool,
    pub empty_reset: String,
    pub max_length: Option<usize>,
    pub verify_value: Option<VerifyFn>,
    pub callback: Option<StrFn>,
    pub disabled: bool,
    pub visible: bool,
    pub tooltip: Option<String>,
    pub disabled_tooltip: Option<String>,
}

impl Default for InputInfo {
    fn default() -> Self {
        Self {
            text: "Input".to_owned(),
            default: String::new(),
            finished: false,
            numeric: false,
            clear_text_on_focus: true,
            clear_text_on_blur: false,
            placeholder: String::new(),
            allow_empty: true,
            empty_reset: "---".to_owned(),
            max_length: None,
            verify_value: None,
            callback: None,
            disabled: false,
            visible: true,
            tooltip: None,
            disabled_tooltip: None,
        }
    }
}

impl InputInfo {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Default::default() }
    }
}

pub struct SliderInfo {
    pub text: String,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub rounding: u8,
    pub prefix: String,
    pub suffix: String,
    pub compact: bool,
    pub hide_max: bool,
    pub format_display_value: Option<FormatF64Fn>,
    pub allow_right_click_input: bool,
    pub callback: Option<F64Fn>,
    pub disabled: bool,
    pub visible: bool,
    pub tooltip: Option<String>,
    pub disabled_tooltip: Option<String>,
}

impl Default for SliderInfo {
    fn default() -> Self {
        Self {
            text: "Slider".to_owned(),
            default: 0.0,
            min: 0.0,
            max: 100.0,
            rounding: 0,
            prefix: String::new(),
            suffix: String::new(),
            compact: false,
            hide_max: false,
            format_display_value: None,
            allow_right_click_input: true,
            callback: None,
            disabled: false,
            visible: true,
            tooltip: None,
            disabled_tooltip: None,
        }
    }
}

impl SliderInfo {
    pub fn new(text: impl Into<String>, min: f64, max: f64, default: f64) -> Self {
        Self { text: text.into(), min, max, default, ..Default::default() }
    }
    pub fn rounding(mut self, r: u8) -> Self {
        self.rounding = r;
        self
    }
    pub fn suffix(mut self, s: impl Into<String>) -> Self {
        self.suffix = s.into();
        self
    }
}

pub struct DropdownInfo {
    pub text: Option<String>,
    pub values: Vec<DropdownEntry>,
    pub disabled_values: Vec<String>,
    pub value_images: std::collections::HashMap<String, IconRef>,
    pub multi: bool,
    pub drag_select: bool,
    pub max_visible_dropdown_items: usize,
    pub select_all_buttons: bool,
    pub expandable: bool,
    pub expand_columns: usize,
    pub searchable: bool,
    pub format_list_value: Option<FormatStrFn>,
    pub format_display_value: Option<FormatStrFn>,
    pub allow_null: bool,
    pub default: DropdownDefault,
    pub callback: Option<DropFn>,
    pub disabled: bool,
    pub visible: bool,
    pub tooltip: Option<String>,
    pub disabled_tooltip: Option<String>,
}

impl Default for DropdownInfo {
    fn default() -> Self {
        Self {
            text: None,
            values: Vec::new(),
            disabled_values: Vec::new(),
            value_images: Default::default(),
            multi: false,
            drag_select: false,
            max_visible_dropdown_items: 8,
            select_all_buttons: true,
            expandable: true,
            expand_columns: 2,
            searchable: false,
            format_list_value: None,
            format_display_value: None,
            allow_null: false,
            default: DropdownDefault::None,
            callback: None,
            disabled: false,
            visible: true,
            tooltip: None,
            disabled_tooltip: None,
        }
    }
}

impl DropdownInfo {
    pub fn new(text: impl Into<String>, values: impl IntoIterator<Item = impl Into<DropdownEntry>>) -> Self {
        Self { text: Some(text.into()), values: values.into_iter().map(Into::into).collect(), ..Default::default() }
    }
    pub fn multi(mut self) -> Self {
        self.multi = true;
        self
    }
    pub fn searchable(mut self) -> Self {
        self.searchable = true;
        self
    }
    pub fn default_value(mut self, v: impl Into<String>) -> Self {
        self.default = DropdownDefault::Value(v.into());
        self
    }
    pub fn default_values(mut self, v: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.default = DropdownDefault::Values(v.into_iter().map(Into::into).collect());
        self
    }
}

pub struct PriorityDropdownInfo {
    pub text: Option<String>,
    pub values: Vec<String>,
    pub default: Vec<String>,
    pub max_visible_dropdown_items: usize,
    pub searchable: bool,
    pub expandable: bool,
    pub format_display_value: Option<FormatStrFn>,
    pub callback: Option<ListFn>,
    pub disabled: bool,
    pub visible: bool,
    pub tooltip: Option<String>,
    pub disabled_tooltip: Option<String>,
}

impl Default for PriorityDropdownInfo {
    fn default() -> Self {
        Self {
            text: None,
            values: Vec::new(),
            default: Vec::new(),
            max_visible_dropdown_items: 8,
            searchable: true,
            expandable: true,
            format_display_value: None,
            callback: None,
            disabled: false,
            visible: true,
            tooltip: None,
            disabled_tooltip: None,
        }
    }
}

impl PriorityDropdownInfo {
    pub fn new(text: impl Into<String>, values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { text: Some(text.into()), values: values.into_iter().map(Into::into).collect(), ..Default::default() }
    }
}

pub struct ImageInfo {
    pub image: IconRef,
    pub transparency: f32,
    pub background_transparency: f32,
    pub color: Color32,
    pub uv: Option<Rect>,
    pub scale_type: ScaleType,
    pub height: f32,
    pub visible: bool,
}

impl Default for ImageInfo {
    fn default() -> Self {
        Self {
            image: IconRef::Lucide("image".to_owned()),
            transparency: 0.0,
            background_transparency: 0.0,
            color: Color32::WHITE,
            uv: None,
            scale_type: ScaleType::Fit,
            height: 200.0,
            visible: true,
        }
    }
}

#[derive(Default)]
pub struct ProfileCardInfo {
    pub avatar: Option<IconRef>,
    pub name: String,
    pub title: String,
    pub description: Description,
    pub height: Option<f32>,
    pub visible: bool,
}

impl ProfileCardInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), visible: true, ..Default::default() }
    }
}

pub struct CustomInfo {
    pub height: f32,
    pub visible: bool,
    pub draw: CustomDrawFn,
}

impl CustomInfo {
    pub fn new(height: f32, draw: impl FnMut(&mut crate::handles::CustomCtx<'_>) + Send + 'static) -> Self {
        Self { height, visible: true, draw: Box::new(draw) }
    }
}

pub struct KeyPickerInfo {
    pub text: String,
    pub default: KeyValue,
    pub default_modifiers: Vec<Modifier>,
    pub blacklisted: Vec<KeyValue>,
    pub blacklisted_modifiers: Vec<Modifier>,
    pub whitelisted: Vec<KeyValue>,
    pub whitelisted_modifiers: Vec<Modifier>,
    pub mode: KeyMode,
    pub modes: Vec<KeyMode>,
    pub sync_toggle_state: bool,
    pub no_ui: bool,
    pub wait_for_callback: bool,
    pub callback: Option<BoolFn>,
    pub clicked: Option<BoolFn>,
    pub changed: Option<KeyFn>,
}

impl Default for KeyPickerInfo {
    fn default() -> Self {
        Self {
            text: "KeyPicker".to_owned(),
            default: KeyValue::None,
            default_modifiers: Vec::new(),
            blacklisted: Vec::new(),
            blacklisted_modifiers: Vec::new(),
            whitelisted: Vec::new(),
            whitelisted_modifiers: Vec::new(),
            mode: KeyMode::Toggle,
            modes: vec![KeyMode::Always, KeyMode::Toggle, KeyMode::Hold],
            sync_toggle_state: false,
            no_ui: false,
            wait_for_callback: false,
            callback: None,
            clicked: None,
            changed: None,
        }
    }
}

impl KeyPickerInfo {
    pub fn new(text: impl Into<String>, default: KeyValue) -> Self {
        Self { text: text.into(), default, ..Default::default() }
    }
    pub fn key(text: impl Into<String>, key: Key) -> Self {
        Self::new(text, KeyValue::Key(key))
    }
    pub fn mode(mut self, mode: KeyMode) -> Self {
        self.mode = mode;
        self
    }
    pub fn sync_toggle_state(mut self) -> Self {
        self.sync_toggle_state = true;
        self
    }
}

pub struct ColorPickerInfo {
    pub default: Color32,
    pub transparency: Option<f32>,
    pub title: Option<String>,
    pub resizable: bool,
    pub callback: Option<ColorFn>,
}

impl Default for ColorPickerInfo {
    fn default() -> Self {
        Self { default: Color32::WHITE, transparency: None, title: None, resizable: true, callback: None }
    }
}

impl ColorPickerInfo {
    pub fn new(default: Color32) -> Self {
        Self { default, ..Default::default() }
    }
    pub fn with_alpha(mut self, transparency: f32) -> Self {
        self.transparency = Some(transparency);
        self
    }
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }
}

pub struct NotifyInfo {
    pub title: Option<String>,
    pub title_color: Option<Color32>,
    pub description: String,
    pub description_color: Option<Color32>,
    pub time: f32,
    pub steps: Option<u32>,
    pub persist: bool,
    pub icon: Option<IconRef>,
    pub big_icon: Option<IconRef>,
    pub icon_color: Option<Color32>,
    pub kind: Option<NotifyType>,
}

impl Default for NotifyInfo {
    fn default() -> Self {
        Self {
            title: None,
            title_color: None,
            description: String::new(),
            description_color: None,
            time: 5.0,
            steps: None,
            persist: false,
            icon: None,
            big_icon: None,
            icon_color: None,
            kind: None,
        }
    }
}

impl NotifyInfo {
    pub fn new(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self { title: Some(title.into()), description: description.into(), ..Default::default() }
    }
    pub fn kind(mut self, k: NotifyType) -> Self {
        self.kind = Some(k);
        self
    }
    pub fn time(mut self, secs: f32) -> Self {
        self.time = secs;
        self
    }
}

pub struct LoadingInfo {
    pub title: String,
    pub icon: Option<IconRef>,
    pub icon_size: f32,
    pub loading_icon: IconRef,
    pub loading_icon_color: Option<Color32>,
    pub loading_icon_tween_time: f32,
    pub current_step: u32,
    pub total_steps: u32,
    pub show_sidebar: bool,
    pub auto_resize_height: bool,
    pub always_on_top: bool,
    pub window_width: f32,
    pub window_height: f32,
    pub content_width: f32,
    pub sidebar_width: f32,
}

impl Default for LoadingInfo {
    fn default() -> Self {
        Self {
            title: "mspaint".to_owned(),
            icon: None,
            icon_size: 30.0,
            loading_icon: IconRef::Lucide("loader-circle".to_owned()),
            loading_icon_color: None,
            loading_icon_tween_time: 1.0,
            current_step: 0,
            total_steps: 10,
            show_sidebar: false,
            auto_resize_height: false,
            always_on_top: true,
            window_width: 450.0,
            window_height: 275.0,
            content_width: 450.0,
            sidebar_width: 250.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InfoPoint {
    pub title: Option<String>,
    pub text: String,
}

pub struct UnsupportedInfo {
    pub executor: String,
    pub supported: Option<Vec<String>>,
    pub unsupported: Option<Vec<String>>,
    pub title: String,
    pub icon: Option<IconRef>,
    pub font: Option<FontFamily>,
    pub corner_radius: Option<f32>,
    pub footer: Option<Footer>,
    pub always_on_top: bool,
    pub information: Option<Vec<InfoPoint>>,
}

impl Default for UnsupportedInfo {
    fn default() -> Self {
        Self {
            executor: "Unknown".to_owned(),
            supported: None,
            unsupported: None,
            title: "Unsupported".to_owned(),
            icon: None,
            font: None,
            corner_radius: None,
            footer: None,
            always_on_top: false,
            information: None,
        }
    }
}

pub struct WatermarkSegment {
    pub text: Option<String>,
    pub getter: Option<GetterFn>,
    pub accent: bool,
    pub icon: Option<IconRef>,
    pub avatar: Option<IconRef>,
}

impl WatermarkSegment {
    pub fn text(t: impl Into<String>) -> Self {
        Self { text: Some(t.into()), getter: None, accent: false, icon: None, avatar: None }
    }
    pub fn getter(f: impl Fn() -> String + Send + Sync + 'static) -> Self {
        Self { text: None, getter: Some(Box::new(f)), accent: false, icon: None, avatar: None }
    }
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }
    pub fn icon(mut self, icon: impl Into<IconRef>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    pub fn avatar(mut self, icon: impl Into<IconRef>) -> Self {
        self.avatar = Some(icon.into());
        self
    }
}

impl From<&str> for WatermarkSegment {
    fn from(s: &str) -> Self {
        WatermarkSegment::text(s)
    }
}

pub struct DraggableLabelInfo {
    pub text: String,
    pub icon: Option<IconRef>,
    pub icon_position: IconPosition,
}

impl DraggableLabelInfo {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), icon: None, icon_position: IconPosition::Left }
    }
}

pub struct DraggableButtonInfo {
    pub text: String,
    pub callback: Option<UnitFn>,
    pub exclude_scaling: bool,
    pub exclude_dragging: bool,
}

impl DraggableButtonInfo {
    pub fn new(text: impl Into<String>, f: impl FnMut() + Send + 'static) -> Self {
        Self { text: text.into(), callback: Some(Box::new(f)), exclude_scaling: false, exclude_dragging: false }
    }
}

pub struct DraggableImageButtonInfo {
    pub icon: IconRef,
    pub icon_size: f32,
    pub callback: Option<UnitFn>,
    pub exclude_scaling: bool,
    pub exclude_dragging: bool,
}

impl DraggableImageButtonInfo {
    pub fn new(icon: impl Into<IconRef>, f: impl FnMut() + Send + 'static) -> Self {
        Self { icon: icon.into(), icon_size: 24.0, callback: Some(Box::new(f)), exclude_scaling: false, exclude_dragging: false }
    }
}

/// Value type of the copied color buffer.
pub type CopiedColor = Rgba;
