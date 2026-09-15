//! Obsidian Ultra core: a retained-model, custom-drawn UI library.
//!
//! This crate is renderer-agnostic: it lays out and paints a retained object
//! model (window -> tabs -> groupboxes -> elements) into [`epaint`] shapes each
//! frame, and hands text editing / icon rasterization to a backend through
//! small traits ([`frame::TextLayout`], [`frame::IconResolver`]).
//!
//! The public API mirrors the original Obsidian Roblox library: create a
//! [`Ui`], call [`Ui::create_window`], add tabs, groupboxes and elements, and
//! keep the returned handles to read values or register listeners.

pub mod color;
pub mod draw;
pub mod frame;
pub mod input;
pub mod layout;
pub mod richtext;
pub mod scheme;
pub mod search;
pub mod testing;
pub mod tween;
pub mod types;

mod elements;
mod handles;
mod node;
mod overlays;
mod ui;
mod window;

pub use color::Rgba;
pub use frame::{
    CursorRequest, FocusRequest, FrameInput, FrameOutput, IconResolver, Layer, TextFieldRequest,
    TextFieldResponse, TextLayout,
};
pub use handles::*;
pub use input::{Input, Key, Modifier, MouseButton};
pub use node::NodeId;
pub use scheme::Scheme;
pub use types::*;
pub use ui::{Animations, HistoryEntry, Ui, UiSettings};

pub use epaint;
pub use epaint::emath;
pub use epaint::{Color32, FontFamily, FontId, Pos2, Rect, Vec2};
