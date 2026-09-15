//! Image, ProfileCard (compact + banner) and Custom (draw closure) elements.

use epaint::emath::Align;
use epaint::{Pos2, Rect, StrokeKind, Vec2};

use crate::draw::{self, Corners};
use crate::frame::{self, Cx};
use crate::handles::info::{CustomInfo, ImageInfo, ProfileCardInfo};
use crate::handles::CustomCtx;
use crate::node::*;
use crate::types::{Description, DescriptionLine, IconRef, ScaleType};
use crate::ui::Model;

const PROFILE_LINE_H: f32 = 15.0;
const PROFILE_LINE_PAD: f32 = 2.0;
const PROFILE_DIVIDER_H: f32 = 7.0;
const PROFILE_COMPACT_H: f32 = 190.0;
const PROFILE_FULL_H: f32 = 84.0;

pub(crate) fn create_image(m: &mut Model, parent: NodeId, idx: &str, info: ImageInfo) -> NodeId {
    let data = ImageData {
        image: info.image,
        transparency: info.transparency,
        background_transparency: info.background_transparency,
        color: info.color,
        uv: info.uv,
        scale_type: info.scale_type,
        height: info.height.max(1.0),
    };
    let id = m.insert(Kind::Image(data), Some(parent));
    m.nodes[id].visible = info.visible;
    m.add_child(parent, id);
    m.register_option(idx, id);
    id
}

/// `compact == true` -> groupbox card; `false` -> full-width tab banner.
pub(crate) fn create_profile(m: &mut Model, parent: NodeId, idx: &str, info: ProfileCardInfo, compact: bool) -> NodeId {
    let data = ProfileData { avatar: info.avatar, name: info.name, title: info.title, description: info.description, height: info.height, compact };
    let id = m.insert(Kind::ProfileCard(data), Some(parent));
    m.nodes[id].visible = info.visible;
    if compact {
        m.add_child(parent, id);
    } else if let Kind::Tab(t) = &mut m.nodes[parent].kind {
        t.banners.push(id);
    }
    m.register_option(idx, id);
    id
}

pub(crate) fn create_custom(m: &mut Model, parent: NodeId, idx: &str, info: CustomInfo) -> NodeId {
    let id = m.insert(Kind::Custom(CustomData { height: info.height.max(1.0), draw: info.draw }), Some(parent));
    m.nodes[id].visible = info.visible;
    m.add_child(parent, id);
    m.register_option(idx, id);
    id
}

// ----- heights -------------------------------------------------------------------------------

pub(crate) fn image_height(m: &Model, id: NodeId, cx: &Cx) -> f32 {
    match &m.nodes[id].kind {
        Kind::Image(i) => cx.px(i.height),
        _ => 0.0,
    }
}

/// One rendered description row of a full banner.
enum ProfileRow {
    Text(String),
    Divider,
}

impl ProfileRow {
    fn height(&self) -> f32 {
        match self {
            ProfileRow::Text(_) => PROFILE_LINE_H,
            ProfileRow::Divider => PROFILE_DIVIDER_H,
        }
    }
}

/// `IsDescriptionDivider` for strings: three or more dashes, optionally padded.
fn is_dash_divider(s: &str) -> bool {
    let t = s.trim();
    t.len() >= 3 && t.bytes().all(|b| b == b'-')
}

fn profile_rows(desc: &Description) -> Vec<ProfileRow> {
    match desc {
        Description::Empty => Vec::new(),
        Description::Text(t) if t.is_empty() => Vec::new(),
        Description::Text(t) => t.lines().map(|l| if is_dash_divider(l) { ProfileRow::Divider } else { ProfileRow::Text(l.to_owned()) }).collect(),
        Description::Lines(lines) => lines
            .iter()
            .map(|l| match l {
                DescriptionLine::Divider => ProfileRow::Divider,
                DescriptionLine::Text(t) if is_dash_divider(t) => ProfileRow::Divider,
                DescriptionLine::Text(t) => ProfileRow::Text(t.clone()),
            })
            .collect(),
    }
}

/// `PlayerInfo:GetTotalHeight()` in design units.
fn profile_total_height(p: &ProfileData, rows: &[ProfileRow]) -> f32 {
    if p.compact {
        return p.height.unwrap_or(PROFILE_COMPACT_H);
    }
    let mut desc_h = 0.0;
    for (i, r) in rows.iter().enumerate() {
        desc_h += r.height();
        if i > 0 {
            desc_h += PROFILE_LINE_PAD;
        }
    }
    p.height.unwrap_or(PROFILE_FULL_H).max(42.0 + desc_h)
}

pub(crate) fn profile_height(m: &mut Model, id: NodeId, cx: &mut Cx, w: f32) -> f32 {
    let _ = w;
    match &m.nodes[id].kind {
        Kind::ProfileCard(p) => {
            let rows = profile_rows(&p.description);
            cx.px(profile_total_height(p, &rows))
        }
        _ => 0.0,
    }
}

pub(crate) fn custom_height(m: &Model, id: NodeId, cx: &Cx) -> f32 {
    match &m.nodes[id].kind {
        Kind::Custom(c) => cx.px(c.height),
        _ => 0.0,
    }
}

// ----- image ---------------------------------------------------------------------------------

pub(crate) fn pass_image(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let Kind::Image(i) = &m.nodes[id].kind else { return 0.0 };
    let (image, transparency, bg_t, color, uv, scale_type, height) =
        (i.image.clone(), i.transparency, i.background_transparency, i.color, i.uv, i.scale_type, i.height);
    let h = cx.px(height);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    m.nodes[id].rect = r;
    cx.rect(r, Corners::ZERO, cx.col(cx.sch.main, bg_t));
    cx.stroke(r, Corners::ZERO, 1.0, cx.col(cx.sch.outline, 0.0), StrokeKind::Inside);
    // Padding T4 R8 B3 L8.
    let inner = Rect::from_min_max(Pos2::new(r.min.x + cx.px(8.0), r.min.y + cx.px(4.0)), Pos2::new(r.max.x - cx.px(8.0), r.max.y - cx.px(3.0)));
    if inner.width() <= 0.0 || inner.height() <= 0.0 {
        return h;
    }
    let px = (inner.width().max(inner.height()) * cx.m.ppp).round().max(1.0) as u32;
    let Some((tex, size)) = cx.resolve_icon(&image, px) else { return h };
    let uv = uv.unwrap_or(draw::FULL_UV);
    let tex_size = Vec2::new(size.x * uv.width(), size.y * uv.height());
    let dst = match scale_type {
        ScaleType::Fit => frame::fit_rect(inner, tex_size),
        ScaleType::Stretch => inner,
        ScaleType::Crop => frame::cover_rect(inner, tex_size),
    };
    let tint = cx.col(color, transparency);
    cx.with_clip(inner, |cx| {
        let s = draw::image(tex, dst, uv, tint);
        cx.push(s);
    });
    h
}

// ----- profile card --------------------------------------------------------------------------

/// Draw the avatar (or the `user` fallback icon at 0.5) fitted into `r`.
fn draw_avatar(cx: &mut Cx, avatar: Option<&IconRef>, r: Rect) {
    if let Some(a) = avatar {
        if cx.icon(a, r, cx.col(cx.sch.white, 0.0)) {
            return;
        }
    }
    cx.icon(&IconRef::Lucide("user".into()), r, cx.col(cx.sch.font_color, 0.5));
}

pub(crate) fn pass_profile(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let Kind::ProfileCard(p) = &m.nodes[id].kind else { return 0.0 };
    let rows = profile_rows(&p.description);
    let total = profile_total_height(p, &rows);
    let (avatar, name, title, base_height, compact) = (p.avatar.clone(), p.name.clone(), p.title.clone(), p.height, p.compact);
    let h = cx.px(total);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    m.nodes[id].rect = r;

    if compact {
        let corners = Corners::same(cx.r2());
        cx.rect(r, corners, cx.col(cx.sch.main, 0.0));
        let inner = r.shrink(cx.px(6.0));
        if inner.is_positive() {
            cx.with_clip(inner, |cx| draw_avatar(cx, avatar.as_ref(), inner));
        }
        cx.outline(r, corners, 0.0);
        return h;
    }

    let corners = Corners::same(cx.r());
    cx.rect(r, corners, cx.col(cx.sch.main, 0.0));
    let pad = cx.px(10.0);
    let avatar_size = cx.px((base_height.unwrap_or(PROFILE_FULL_H) - 20.0).max(0.0));
    let tile = Rect::from_min_size(r.min + Vec2::splat(pad), Vec2::splat(avatar_size));
    cx.with_clip(r, |cx| {
        if tile.is_positive() {
            let tc = Corners::same(cx.r2());
            cx.rect(tile, tc, cx.col(cx.sch.background, 0.0));
            cx.with_clip(tile, |cx| draw_avatar(cx, avatar.as_ref(), tile));
            cx.outline(tile, tc, 0.0);
        }
        let tx = tile.max.x + cx.px(12.0);
        let tw = (r.max.x - pad - tx).max(1.0);
        let title_text = if title.is_empty() { format!("Hello, {name}") } else { title };
        let title_rect = Rect::from_min_size(Pos2::new(tx, r.min.y + pad), Vec2::new(tw, cx.px(18.0)));
        let shown = cx.truncate(&title_text, 15.0, tw, true);
        cx.text(title_rect, &shown, 15.0, cx.col(cx.sch.font_color, 0.0), Align::Min, false, true);
        let mut ry = r.min.y + pad + cx.px(22.0);
        for row in &rows {
            let rh = cx.px(row.height());
            match row {
                ProfileRow::Text(t) => {
                    let rr = Rect::from_min_size(Pos2::new(tx, ry), Vec2::new(tw, rh));
                    let shown = cx.truncate(t, 13.0, tw, true);
                    cx.text(rr, &shown, 13.0, cx.col(cx.sch.font_color, 0.25), Align::Min, false, true);
                }
                ProfileRow::Divider => {
                    cx.hline(tx, tx + tw, ry + rh / 2.0, cx.col(cx.sch.outline, 0.0));
                }
            }
            ry += rh + cx.px(PROFILE_LINE_PAD);
        }
    });
    cx.outline(r, corners, 0.0);
    h
}

// ----- custom --------------------------------------------------------------------------------

pub(crate) fn pass_custom(m: &mut Model, id: NodeId, cx: &mut Cx, x: f32, y: f32, w: f32) -> f32 {
    let h = custom_height(m, id, cx);
    let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
    m.nodes[id].rect = r;
    let hovered = cx.hovered(r);
    let Kind::Custom(c) = &mut m.nodes[id].kind else { return 0.0 };
    // Take the closure out so the model is not borrowed while user code runs.
    let mut draw: CustomDrawFn = std::mem::replace(&mut c.draw, Box::new(|_| {}));
    let mut shapes = Vec::new();
    {
        let input: &crate::input::Input = cx.fi.input;
        let mut ctx = CustomCtx { rect: r, input, scheme: &cx.sch, scale: cx.m.s, time: cx.time, shapes: &mut shapes, hovered };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(&mut ctx)));
    }
    if let Kind::Custom(c) = &mut m.nodes[id].kind {
        c.draw = draw;
    }
    if !shapes.is_empty() {
        cx.with_clip(r, |cx| cx.push_all(shapes));
    }
    h
}
