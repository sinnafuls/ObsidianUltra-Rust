//! Icon resolution (Lucide SVGs, user images) and text layout bridging.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::load::{SizeHint, TexturePoll};
use egui::{TextureId, TextureOptions, Vec2};
use include_dir::{include_dir, Dir};
use obsidian_ultra_core::frame::{IconResolver, TextLayout};
use obsidian_ultra_core::IconRef;

static LUCIDE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/assets/lucide");

/// Bundled Lucide icon set (ISC license, see `assets/lucide/LICENSE`).
pub struct LucideIcons;

impl LucideIcons {
    /// SVG source of a Lucide icon, with `currentColor` replaced by white so tinting works.
    pub fn svg(name: &str) -> Option<String> {
        let file = LUCIDE.get_file(format!("{name}.svg"))?;
        let src = file.contents_utf8()?;
        Some(src.replace("currentColor", "#ffffff"))
    }

    pub fn names() -> impl Iterator<Item = &'static str> {
        LUCIDE.files().filter_map(|f| f.path().file_stem().and_then(|s| s.to_str()))
    }

    pub fn contains(name: &str) -> bool {
        LUCIDE.get_file(format!("{name}.svg")).is_some()
    }
}

type Key = (String, u32);

/// Icon texture cache. egui's texture loader evicts textures that were not polled during a
/// pass, so every icon used in a frame is re-polled (`touch`) before the next frame.
#[derive(Default)]
pub(crate) struct IconCache {
    ready: HashMap<Key, Option<(TextureId, Vec2)>>,
    /// Icon refs behind each key (needed to re-poll).
    refs: HashMap<Key, IconRef>,
    pending: Vec<(IconRef, u32)>,
    pending_keys: HashSet<Key>,
    used: HashSet<Key>,
    registered: HashSet<String>,
}

fn bucket(size_px: u32) -> u32 {
    size_px.clamp(16, 512).div_ceil(8) * 8
}

impl IconCache {
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    fn uri(&mut self, ctx: &egui::Context, icon: &IconRef) -> Option<String> {
        let key = icon.key();
        let uri = match icon {
            IconRef::Lucide(name) => format!("bytes://obsidian/lucide/{name}.svg"),
            IconRef::Bytes { name, .. } => {
                let ext = if name.contains('.') { "" } else { ".png" };
                format!("bytes://obsidian/bytes/{name}{ext}")
            }
            IconRef::Path(p) => format!("file://{}", p.display()),
            IconRef::Texture { .. } => return None,
        };
        if !self.registered.contains(&key) {
            match icon {
                IconRef::Lucide(name) => {
                    let svg = LucideIcons::svg(name)?;
                    ctx.include_bytes(uri.clone(), svg.into_bytes());
                }
                IconRef::Bytes { bytes, .. } => {
                    let v: Vec<u8> = bytes.to_vec();
                    ctx.include_bytes(uri.clone(), v);
                }
                _ => {}
            }
            self.registered.insert(key);
        }
        Some(uri)
    }

    fn poll(&mut self, ctx: &egui::Context, icon: &IconRef, size: u32) -> Poll {
        if let IconRef::Texture { id, size: sz } = icon {
            return Poll::Ready((*id, *sz));
        }
        let Some(uri) = self.uri(ctx, icon) else { return Poll::Failed };
        let hint = SizeHint::Size { width: size, height: size, maintain_aspect_ratio: true };
        match ctx.try_load_texture(&uri, TextureOptions::LINEAR, hint) {
            Ok(TexturePoll::Ready { texture }) => Poll::Ready((texture.id, texture.size)),
            Ok(TexturePoll::Pending { .. }) => Poll::Pending,
            Err(_) => Poll::Failed,
        }
    }

    /// Load icons requested during the last frame and keep the used ones alive.
    /// Must run outside `fonts_mut` (egui `Context` re-entrancy).
    pub fn resolve_pending(&mut self, ctx: &egui::Context) {
        // Keep every icon drawn last frame loaded (egui evicts textures unused for a pass).
        let used: Vec<Key> = self.used.drain().collect();
        for k in used {
            if !matches!(self.ready.get(&k), Some(Some(_))) {
                continue;
            }
            let Some(icon) = self.refs.get(&k).cloned() else { continue };
            match self.poll(ctx, &icon, k.1) {
                Poll::Ready(t) => {
                    self.ready.insert(k, Some(t));
                }
                Poll::Pending => {
                    self.ready.remove(&k);
                    if !self.pending_keys.contains(&k) {
                        self.pending_keys.insert(k.clone());
                        self.pending.push((icon, k.1));
                    }
                }
                Poll::Failed => {
                    self.ready.insert(k, None);
                }
            }
        }
        let pending = std::mem::take(&mut self.pending);
        self.pending_keys.clear();
        for (icon, size) in pending {
            let k = (icon.key(), size);
            self.refs.entry(k.clone()).or_insert_with(|| icon.clone());
            match self.poll(ctx, &icon, size) {
                Poll::Ready(t) => {
                    self.ready.insert(k, Some(t));
                }
                Poll::Pending => {
                    self.pending_keys.insert(k);
                    self.pending.push((icon, size));
                }
                Poll::Failed => {
                    self.ready.insert(k, None);
                }
            }
        }
    }
}

enum Poll {
    Ready((TextureId, Vec2)),
    Pending,
    Failed,
}

impl IconResolver for IconCache {
    fn resolve(&mut self, icon: &IconRef, size_px: u32) -> Option<(TextureId, Vec2)> {
        let size = bucket(size_px);
        let k = (icon.key(), size);
        if let Some(r) = self.ready.get(&k) {
            if r.is_some() {
                self.used.insert(k);
            }
            return *r;
        }
        if !self.pending_keys.contains(&k) {
            self.pending_keys.insert(k.clone());
            self.pending.push((icon.clone(), size));
        }
        None
    }
}

/// `TextLayout` over egui's `FontsView`.
pub(crate) struct EguiText<'a, 'b>(pub &'a mut egui::epaint::text::FontsView<'b>);

impl TextLayout for EguiText<'_, '_> {
    fn layout(&mut self, job: egui::epaint::text::LayoutJob) -> Arc<egui::epaint::Galley> {
        self.0.layout_job(job)
    }
}
