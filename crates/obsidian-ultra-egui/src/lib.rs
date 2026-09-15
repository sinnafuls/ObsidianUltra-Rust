//! egui backend for Obsidian Ultra.
//!
//! ```no_run
//! # use obsidian_ultra_core::*;
//! let ui = Ui::new();
//! let mut obsidian = obsidian_ultra_egui::Obsidian::new(ui.clone());
//! // inside your egui frame:
//! # let ctx = egui::Context::default();
//! obsidian.show(&ctx);
//! ```
//!
//! Call [`Obsidian::show`] once per egui frame (from any egui host: eframe, a DirectX hook, …).
//! The core lays out and paints into egui's painter; text fields are hosted as egui
//! `TextEdit`s so editing, IME and clipboard work natively.

mod icons;
mod keys;
#[cfg(windows)]
mod rawkeys;

use std::collections::{HashMap, HashSet};

use egui::{Align, Area, Id, Order, Pos2, Rect, Sense, TextEdit, Vec2};
use obsidian_ultra_core::frame::{CursorRequest, FocusRequest, FrameInput, TextFieldResponse};
use obsidian_ultra_core::input::{Input, Key};
use obsidian_ultra_core::{NodeId, Ui};

pub use icons::LucideIcons;

/// Optional raw key-state provider (left/right modifier keys, keys while unfocused).
/// The backend layers it on top of egui's own key state.
pub trait RawKeys: Send {
    fn is_down(&mut self, key: Key) -> bool;
}

#[derive(Default)]
struct FieldMem {
    text: String,
    focused: bool,
}

/// Hosts an Obsidian [`Ui`] inside egui.
pub struct Obsidian {
    pub ui: Ui,
    responses: Vec<TextFieldResponse>,
    prev_keys: HashSet<Key>,
    raw: Option<Box<dyn RawKeys>>,
    fields: HashMap<(NodeId, u8), FieldMem>,
    icons: icons::IconCache,
    loaders_installed: bool,
    /// Screen rect override (e.g. the game viewport in an overlay); defaults to the egui content rect.
    pub screen: Option<Rect>,
}

impl Obsidian {
    pub fn new(ui: Ui) -> Self {
        Self {
            ui,
            responses: Vec::new(),
            prev_keys: HashSet::new(),
            #[cfg(windows)]
            raw: Some(Box::new(rawkeys::WindowsRawKeys)),
            #[cfg(not(windows))]
            raw: None,
            fields: HashMap::new(),
            icons: icons::IconCache::default(),
            loaders_installed: false,
            screen: None,
        }
    }

    /// Replace (or disable with `None`) the raw key provider.
    pub fn set_raw_keys(&mut self, raw: Option<Box<dyn RawKeys>>) {
        self.raw = raw;
    }

    /// Run one Obsidian frame inside the current egui frame.
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.loaders_installed {
            egui_extras::install_image_loaders(ctx);
            self.loaders_installed = true;
        }
        let screen = self.screen.unwrap_or_else(|| ctx.content_rect());
        let input = self.build_input(ctx);
        let (time, dt, ppp) = ctx.input(|i| (i.time, i.stable_dt, i.pixels_per_point()));

        // Resolve icons requested last frame outside the font lock (Context re-entrancy).
        self.icons.resolve_pending(ctx);

        let responses = std::mem::take(&mut self.responses);
        let ui = self.ui.clone();
        let icons = &mut self.icons;
        let out = ctx.fonts_mut(|fonts| {
            let mut text = icons::EguiText(fonts);
            ui.frame(FrameInput {
                screen,
                pixels_per_point: ppp,
                time,
                dt,
                input: &input,
                text_fields: &responses,
                fonts: &mut text,
                icons,
            })
        });
        if self.icons.has_pending() {
            ctx.request_repaint();
        }

        // Paint every layer, bottom to top, into one egui layer.
        let paint_layer = egui::LayerId::new(Order::Foreground, Id::new("obsidian_paint"));
        let fields_layer = egui::LayerId::new(Order::Foreground, Id::new("obsidian_fields"));
        Area::new(Id::new("obsidian_paint")).order(Order::Foreground).fixed_pos(Pos2::ZERO).interactable(!out.interactive_rects.is_empty()).show(ctx, |ui| {
            // Claim the interactive rects so egui widgets underneath do not react.
            for r in &out.interactive_rects {
                ui.allocate_rect(*r, Sense::click_and_drag());
            }
            ui.allocate_rect(Rect::from_min_size(Pos2::ZERO, Vec2::ZERO), Sense::hover());
        });
        ctx.graphics_mut(|g| {
            let list = g.entry(paint_layer);
            for layer in out.layers.iter() {
                for cs in layer {
                    list.add(cs.clip_rect, cs.shape.clone());
                }
            }
        });

        // Hosted text fields.
        let mut responses = Vec::with_capacity(out.text_fields.len());
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let live: HashSet<(NodeId, u8)> = out.text_fields.iter().map(|f| (f.id, f.sub)).collect();
        self.fields.retain(|k, _| live.contains(k));
        if !out.text_fields.is_empty() {
            Area::new(Id::new("obsidian_fields")).order(Order::Foreground).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                for req in &out.text_fields {
                    let key = (req.id, req.sub);
                    let mem = self.fields.entry(key).or_default();
                    if !mem.focused && mem.text != req.text {
                        mem.text = req.text.clone();
                    }
                    let id = Id::new(("obsidian_field", req.id, req.sub));
                    let hint = egui::RichText::new(&req.placeholder).color(req.placeholder_color).font(req.font.clone());
                    let mut edit = TextEdit::singleline(&mut mem.text)
                        .id(id)
                        .frame(egui::Frame::NONE)
                        .font(req.font.clone())
                        .text_color(req.color)
                        .hint_text(hint)
                        .desired_width(req.rect.width())
                        .vertical_align(Align::Center)
                        .clip_text(true);
                    if req.password {
                        edit = edit.password(true);
                    }
                    if let Some(n) = req.max_len {
                        edit = edit.char_limit(n);
                    }
                    let resp = ui.put(req.rect, edit);
                    match req.focus {
                        FocusRequest::Take => resp.request_focus(),
                        FocusRequest::Release => resp.surrender_focus(),
                        FocusRequest::None => {}
                    }
                    if resp.gained_focus() && req.clear_on_focus {
                        mem.text.clear();
                    }
                    let focused = resp.has_focus();
                    let lost = resp.lost_focus();
                    responses.push(TextFieldResponse {
                        id: req.id,
                        sub: req.sub,
                        text: mem.text.clone(),
                        focused,
                        submitted: lost && enter,
                        lost_focus: lost,
                        gained_focus: resp.gained_focus(),
                    });
                    mem.focused = focused;
                }
            });
            ctx.move_to_top(fields_layer);
        }
        self.responses = responses;

        for s in out.copy_to_clipboard {
            ctx.copy_text(s);
        }
        if out.cursor == CursorRequest::Hidden {
            ctx.set_cursor_icon(egui::CursorIcon::None);
        }
        if let Some(secs) = out.repaint_after {
            ctx.request_repaint_after_secs(secs);
        }
    }

    fn build_input(&mut self, ctx: &egui::Context) -> Input {
        let mut keys_down: HashSet<Key> = HashSet::new();
        let (pointer, delta, down, pressed, released, dbl, scroll, focused, modifiers) = ctx.input(|i| {
            for k in &i.keys_down {
                if let Some(k) = keys::from_egui(*k) {
                    keys_down.insert(k);
                }
            }
            let p = &i.pointer;
            let btn = |b: egui::PointerButton| (p.button_down(b), p.button_pressed(b), p.button_released(b), p.button_double_clicked(b));
            let l = btn(egui::PointerButton::Primary);
            let r = btn(egui::PointerButton::Secondary);
            let mid = btn(egui::PointerButton::Middle);
            (
                p.latest_pos(),
                p.delta(),
                [l.0, r.0, mid.0],
                [l.1, r.1, mid.1],
                [l.2, r.2, mid.2],
                [l.3, r.3, mid.3],
                i.smooth_scroll_delta,
                i.focused,
                i.modifiers,
            )
        });
        // Modifier keys are not delivered as keys by egui; derive left variants from the modifier state.
        if modifiers.ctrl {
            keys_down.insert(Key::LeftControl);
        }
        if modifiers.shift {
            keys_down.insert(Key::LeftShift);
        }
        if modifiers.alt {
            keys_down.insert(Key::LeftAlt);
        }
        if let Some(raw) = &mut self.raw {
            for k in obsidian_ultra_core::input::ALL_KEYS {
                if raw.is_down(*k) {
                    keys_down.insert(*k);
                } else if matches!(k, Key::LeftControl | Key::RightControl | Key::LeftShift | Key::RightShift | Key::LeftAlt | Key::RightAlt) && !modifier_from_egui(&modifiers, *k) {
                    keys_down.remove(k);
                }
            }
        }
        let keys_pressed: Vec<Key> = keys_down.iter().copied().filter(|k| !self.prev_keys.contains(k)).collect();
        let keys_released: Vec<Key> = self.prev_keys.iter().copied().filter(|k| !keys_down.contains(k)).collect();
        self.prev_keys = keys_down.clone();
        Input {
            pointer,
            pointer_delta: delta,
            down,
            pressed,
            released,
            double_clicked: dbl,
            keys_down,
            keys_pressed,
            keys_released,
            scroll,
            window_focused: focused,
            text_edit_focused: ctx.text_edit_focused(),
        }
    }
}

fn modifier_from_egui(m: &egui::Modifiers, k: Key) -> bool {
    match k {
        Key::LeftControl | Key::RightControl => m.ctrl,
        Key::LeftShift | Key::RightShift => m.shift,
        Key::LeftAlt | Key::RightAlt => m.alt,
        _ => false,
    }
}
