//! Small public value types shared by the whole API.

use std::path::PathBuf;
use std::sync::Arc;

use epaint::{Color32, Rect, TextureId, Vec2};

use crate::color::Rgba;
use crate::input::{Key, Modifier, MouseButton};

/// A plain color (re-export of `epaint::Color32`).
pub type Color = Color32;

/// Column of a tab a groupbox/tabbox is placed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Left,
    Right,
}

/// Column layout of a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Layout {
    #[default]
    Dual,
    Single,
}

/// Direction a tab canvas swipes in from when `Animations::tab_switch` is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SwipeFrom {
    Left,
    Right,
    Top,
    #[default]
    Bottom,
}

/// Horizontal alignment (sub-tab bar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

/// Where a draggable label's icon sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IconPosition {
    #[default]
    Left,
    Right,
}

/// How an image is fitted into its box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScaleType {
    #[default]
    Fit,
    Stretch,
    Crop,
}

/// Notification category; selects a primary text color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotifyType {
    Error,
    Warning,
    Success,
    Info,
}

impl NotifyType {
    pub fn label(self) -> &'static str {
        match self {
            NotifyType::Error => "ERROR",
            NotifyType::Warning => "WARNING",
            NotifyType::Success => "SUCCESS",
            NotifyType::Info => "INFO",
        }
    }
}

/// Key-picker activation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KeyMode {
    Always,
    #[default]
    Toggle,
    Hold,
    /// Only valid on Label/Button hosts: fires once per press.
    Press,
}

impl KeyMode {
    pub fn label(self) -> &'static str {
        match self {
            KeyMode::Always => "Always",
            KeyMode::Toggle => "Toggle",
            KeyMode::Hold => "Hold",
            KeyMode::Press => "Press",
        }
    }
}

/// The bound key of a key picker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KeyValue {
    #[default]
    None,
    Unknown,
    Mouse(MouseButton),
    Key(Key),
}

impl KeyValue {
    pub fn display_name(self) -> String {
        match self {
            KeyValue::None => "None".to_owned(),
            KeyValue::Unknown => "Unknown".to_owned(),
            KeyValue::Mouse(b) => b.name().to_owned(),
            KeyValue::Key(k) => k.display_name(),
        }
    }
}

/// Complete key-picker value.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct KeyBind {
    pub key: KeyValue,
    pub modifiers: Vec<Modifier>,
    pub mode: KeyMode,
}

impl KeyBind {
    pub fn display_value(&self) -> String {
        if self.modifiers.is_empty() {
            return self.key.display_name();
        }
        let mut s = String::new();
        for m in &self.modifiers {
            s.push_str(m.short_name());
            s.push_str(" + ");
        }
        s.push_str(&self.key.display_name());
        s
    }
}

/// Style variant of dialog / loading-error footer buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Destructive,
    Ghost,
}

/// Toggle visual variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleVariant {
    Switch,
    Checkbox,
}

/// A reference to an icon or image. Resolution to a texture happens in the backend.
#[derive(Clone, Debug, PartialEq)]
pub enum IconRef {
    /// A Lucide icon name, e.g. `"home"`.
    Lucide(String),
    /// A texture the host already uploaded.
    Texture { id: TextureId, size: Vec2 },
    /// Encoded image bytes (PNG/JPEG/SVG); `name` must be unique per distinct image.
    Bytes { name: String, bytes: Arc<[u8]> },
    /// An image file on disk.
    Path(PathBuf),
}

impl IconRef {
    /// Stable identity used as a cache key by backends.
    pub fn key(&self) -> String {
        match self {
            IconRef::Lucide(n) => format!("lucide/{n}"),
            IconRef::Texture { id, .. } => format!("tex/{id:?}"),
            IconRef::Bytes { name, .. } => format!("bytes/{name}"),
            IconRef::Path(p) => format!("path/{}", p.display()),
        }
    }
}

impl From<&str> for IconRef {
    fn from(s: &str) -> Self {
        IconRef::Lucide(s.to_owned())
    }
}

impl From<String> for IconRef {
    fn from(s: String) -> Self {
        IconRef::Lucide(s)
    }
}

/// Window footer.
#[derive(Clone, Debug, PartialEq)]
pub enum Footer {
    Text(String),
    Segments(Vec<FooterSegment>),
}

impl Default for Footer {
    fn default() -> Self {
        Footer::Text("No Footer".to_owned())
    }
}

impl From<&str> for Footer {
    fn from(s: &str) -> Self {
        Footer::Text(s.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct FooterSegment {
    pub text: String,
    pub copyable: bool,
    pub copy_text: Option<String>,
}

/// Value of a single- or multi-select dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropdownValue {
    Single(Option<String>),
    Multi(std::collections::BTreeSet<String>),
}

impl DropdownValue {
    pub fn contains(&self, v: &str) -> bool {
        match self {
            DropdownValue::Single(Some(s)) => s == v,
            DropdownValue::Single(None) => false,
            DropdownValue::Multi(set) => set.contains(v),
        }
    }

    pub fn active_values(&self) -> Vec<String> {
        match self {
            DropdownValue::Single(Some(s)) => vec![s.clone()],
            DropdownValue::Single(None) => Vec::new(),
            DropdownValue::Multi(set) => set.iter().cloned().collect(),
        }
    }

    pub fn count(&self) -> usize {
        match self {
            DropdownValue::Single(Some(_)) => 1,
            DropdownValue::Single(None) => 0,
            DropdownValue::Multi(set) => set.len(),
        }
    }
}

/// One dropdown entry: the stored value and an optional display text (Lua dict form).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropdownEntry {
    pub value: String,
    pub display: Option<String>,
}

impl DropdownEntry {
    pub fn new(v: impl Into<String>) -> Self {
        Self { value: v.into(), display: None }
    }
    pub fn with_display(v: impl Into<String>, d: impl Into<String>) -> Self {
        Self { value: v.into(), display: Some(d.into()) }
    }
    pub fn display_text(&self) -> &str {
        self.display.as_deref().unwrap_or(&self.value)
    }
}

impl From<&str> for DropdownEntry {
    fn from(s: &str) -> Self {
        DropdownEntry::new(s)
    }
}

impl From<String> for DropdownEntry {
    fn from(s: String) -> Self {
        DropdownEntry::new(s)
    }
}

impl From<(&str, &str)> for DropdownEntry {
    fn from((v, d): (&str, &str)) -> Self {
        DropdownEntry::with_display(v, d)
    }
}

/// Initial dropdown selection.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum DropdownDefault {
    #[default]
    None,
    Value(String),
    Values(Vec<String>),
    Index(usize),
}

/// Value of any registered option (for consumer-side persistence).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    None,
    Text(String),
    Number(f64),
    Dropdown(DropdownValue),
    Priority(Vec<String>),
    Key(KeyBind),
    Color(Rgba),
}

/// Profile-card description.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum Description {
    #[default]
    Empty,
    Text(String),
    Lines(Vec<DescriptionLine>),
}

impl From<&str> for Description {
    fn from(s: &str) -> Self {
        Description::Text(s.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DescriptionLine {
    Text(String),
    Divider,
}

/// Glow options for [`crate::Window::set_glow`].
#[derive(Clone, Debug, Default)]
pub struct GlowOptions {
    pub color: Option<Color>,
    pub transparency: Option<f32>,
    pub radius: Option<f32>,
}

/// Optional sub-rectangle (UV) of an image, in 0..1 coordinates.
pub type Uv = Rect;
