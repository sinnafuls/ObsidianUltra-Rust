//! Headless test harness: runs frames with real epaint text layout and no renderer.
//! Intended for integration tests and host-side smoke tests.

use std::collections::HashMap;
use std::sync::Arc;

use epaint::text::{FontDefinitions, Fonts, LayoutJob, TextOptions};
use epaint::{Galley, Pos2, Rect, TextureId, Vec2};

use crate::frame::{FrameInput, FrameOutput, IconResolver, TextFieldResponse, TextLayout};
use crate::input::{Input, Key, MouseButton};
use crate::types::IconRef;
use crate::{NodeId, Ui};

struct Text<'a>(epaint::text::FontsView<'a>);

impl TextLayout for Text<'_> {
    fn layout(&mut self, job: LayoutJob) -> Arc<Galley> {
        self.0.layout_job(job)
    }
}

/// Resolves every icon to a dummy texture so layout proceeds as in a real backend.
#[derive(Default)]
pub struct DummyIcons {
    pub requested: Vec<String>,
}

impl IconResolver for DummyIcons {
    fn resolve(&mut self, icon: &IconRef, _size_px: u32) -> Option<(TextureId, Vec2)> {
        self.requested.push(icon.key());
        Some((TextureId::Managed(0), Vec2::splat(24.0)))
    }
}

/// Drives a [`Ui`] frame by frame.
pub struct Harness {
    pub ui: Ui,
    pub fonts: Fonts,
    pub input: Input,
    pub responses: Vec<TextFieldResponse>,
    pub time: f64,
    pub screen: Rect,
    pub last: Option<FrameOutput>,
    pub icons: DummyIcons,
    /// Text typed into hosted fields, keyed by (node, sub).
    pub field_text: HashMap<(NodeId, u8), String>,
}

impl Harness {
    pub fn new(ui: Ui) -> Self {
        let fonts = Fonts::new(TextOptions::default(), FontDefinitions::default());
        Self {
            ui,
            fonts,
            input: Input { window_focused: true, ..Default::default() },
            responses: Vec::new(),
            time: 1.0,
            screen: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)),
            last: None,
            icons: DummyIcons::default(),
            field_text: HashMap::new(),
        }
    }

    /// Run one frame with the current input; advances time by `dt`.
    pub fn frame(&mut self, dt: f32) -> &FrameOutput {
        self.time += dt as f64;
        let responses = std::mem::take(&mut self.responses);
        let out = {
            let mut text = Text(self.fonts.with_pixels_per_point(1.0));
            self.ui.frame(FrameInput {
                screen: self.screen,
                pixels_per_point: 1.0,
                time: self.time,
                dt,
                input: &self.input,
                text_fields: &responses,
                fonts: &mut text,
                icons: &mut self.icons,
            })
        };
        // Echo hosted text fields back as a backend would (with any typed text applied).
        self.responses = out
            .text_fields
            .iter()
            .map(|f| {
                let text = self.field_text.get(&(f.id, f.sub)).cloned().unwrap_or_else(|| f.text.clone());
                TextFieldResponse { id: f.id, sub: f.sub, text, focused: false, submitted: false, lost_focus: false, gained_focus: false }
            })
            .collect();
        // One-frame events are consumed.
        self.input.pressed = [false; 3];
        self.input.released = [false; 3];
        self.input.double_clicked = [false; 3];
        self.input.keys_pressed.clear();
        self.input.keys_released.clear();
        self.input.scroll = Vec2::ZERO;
        self.last = Some(out);
        self.last.as_ref().unwrap()
    }

    /// Run `n` idle frames (lets animations settle).
    pub fn settle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(1.0 / 60.0);
        }
    }

    pub fn move_to(&mut self, pos: Pos2) {
        self.input.pointer = Some(pos);
    }

    /// Press + release the left button at `pos` over two frames.
    pub fn click(&mut self, pos: Pos2) {
        self.press(pos, MouseButton::Left);
        self.frame(1.0 / 60.0);
        self.release(MouseButton::Left);
        self.frame(1.0 / 60.0);
    }

    pub fn press(&mut self, pos: Pos2, b: MouseButton) {
        self.input.pointer = Some(pos);
        self.input.down[b.index()] = true;
        self.input.pressed[b.index()] = true;
    }

    pub fn release(&mut self, b: MouseButton) {
        self.input.down[b.index()] = false;
        self.input.released[b.index()] = true;
    }

    /// Press and release a key over one frame each.
    pub fn tap(&mut self, key: Key) {
        self.key_down(key);
        self.frame(1.0 / 60.0);
        self.key_up(key);
        self.frame(1.0 / 60.0);
    }

    pub fn key_down(&mut self, key: Key) {
        self.input.keys_down.insert(key);
        self.input.keys_pressed.push(key);
    }

    pub fn key_up(&mut self, key: Key) {
        self.input.keys_down.remove(&key);
        self.input.keys_released.push(key);
    }

    /// Submit text into a hosted field on the next frame (as if typed + Enter).
    pub fn type_into(&mut self, id: NodeId, sub: u8, text: &str, submit: bool) {
        self.field_text.insert((id, sub), text.to_owned());
        self.frame(1.0 / 60.0);
        if submit {
            if let Some(r) = self.responses.iter_mut().find(|r| r.id == id && r.sub == sub) {
                r.submitted = true;
                r.lost_focus = true;
            }
        }
        self.frame(1.0 / 60.0);
        self.field_text.remove(&(id, sub));
    }

    /// Screen rect of a node from the last frame.
    pub fn rect(&self, id: NodeId) -> Rect {
        self.ui.node_rect(id)
    }

    pub fn center(&self, id: NodeId) -> Pos2 {
        self.rect(id).center()
    }
}

impl Ui {
    /// Screen rect a node occupied during the last frame (testing/debugging aid).
    pub fn node_rect(&self, id: NodeId) -> Rect {
        self.with(|m| m.nodes.get(id).map(|n| n.rect).unwrap_or(Rect::NOTHING))
    }

    /// Lua-style type name of a node (`"Toggle"`, `"Dropdown"`, ...), for tests and debugging.
    pub fn node_type(&self, id: NodeId) -> Option<&'static str> {
        self.with(|m| m.nodes.get(id).map(|n| n.kind.type_name()))
    }

    /// Whether a node and all of its ancestors are shown (visible, not filtered by search, not destroyed).
    pub fn node_shown(&self, id: NodeId) -> bool {
        self.with(|m| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let Some(n) = m.nodes.get(c) else { return false };
                if !n.shown() {
                    return false;
                }
                cur = n.parent;
            }
            true
        })
    }
}
