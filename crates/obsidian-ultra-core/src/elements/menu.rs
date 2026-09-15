//! Context-menu helper: anchored popup with single-open rule,
//! click-outside close and optional open/close height animation.

use epaint::{Pos2, Rect, Vec2};

use crate::draw::Corners;
use crate::frame::{Cx, Layer};
use crate::input::MouseButton;
use crate::node::*;
use crate::tween::Ease;
use crate::ui::Model;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuCorners {
    Bottom,
    NoLeft,
    NoTopLeft,
}

impl MenuCorners {
    pub fn corners(self, r: f32) -> Corners {
        match self {
            MenuCorners::Bottom => Corners::bottom(r),
            MenuCorners::NoLeft => Corners::no_left(r),
            MenuCorners::NoTopLeft => Corners::no_top_left(r),
        }
    }
}

pub(crate) struct MenuSpec {
    pub holder: Rect,
    /// Width in points.
    pub width: f32,
    /// Offset from the holder's top-left, in points.
    pub offset: Vec2,
    /// Full content height in points.
    pub height: f32,
    /// Animation (duration, easing) when the matching `Animations` flag is on.
    pub anim: Option<(f32, Ease)>,
}

/// Toggle a menu open/closed.
pub(crate) fn toggle(m: &mut Model, id: NodeId, sub: u8, state: &mut MenuState) {
    if state.open {
        close(m, id, sub, state);
    } else {
        open(m, id, sub, state);
    }
}

pub(crate) fn open(m: &mut Model, id: NodeId, sub: u8, state: &mut MenuState) {
    if state.open {
        return;
    }
    crate::window::open_menu(m, id, sub);
    state.open = true;
    state.opened_at = m.time;
}

pub(crate) fn close(m: &mut Model, id: NodeId, sub: u8, state: &mut MenuState) {
    if !state.open {
        return;
    }
    state.open = false;
    if m.current_menu == Some((id, sub)) {
        m.current_menu = None;
    }
}

/// Resolve the menu rect for this frame, handle click-outside, animate.
/// Returns `Some(rect)` while the menu is visible (open or closing).
pub(crate) fn begin(m: &mut Model, id: NodeId, sub: u8, cx: &mut Cx, state: &mut MenuState, spec: MenuSpec) -> Option<Rect> {
    // Another menu took over the single-open slot.
    if state.open && m.current_menu != Some((id, sub)) {
        state.open = false;
    }
    if state.open && !m.open && !crate::overlays::loading_owns(m, id) {
        state.open = false;
        if m.current_menu == Some((id, sub)) {
            m.current_menu = None;
        }
    }
    let target = if state.open { 1.0 } else { 0.0 };
    match spec.anim {
        Some((dur, ease)) => {
            state.anim.set(target, cx.time, dur, ease);
        }
        None => state.anim.snap(target),
    }
    let progress = cx.anim_value(&state.anim);
    if progress <= 0.001 && !state.open {
        state.rect = Rect::NOTHING;
        return None;
    }
    let full_h = spec.height.max(0.0);
    let h = full_h * progress;
    let origin = spec.holder.min + spec.offset;
    let rect = Rect::from_min_size(Pos2::new(origin.x.floor(), origin.y.floor()), Vec2::new(spec.width, h));
    state.rect = rect;
    state.holder = spec.holder;

    // Auto-close when the holder scrolled out of view (Lua: not inside the window container).
    if state.open && !cx.clip.intersects(spec.holder) {
        close(m, id, sub, state);
    }

    // Click-outside close (any mouse button), ignoring the opening click.
    if state.open && state.opened_at != cx.time {
        let pressed = cx.pressed(MouseButton::Left) || cx.pressed(MouseButton::Right) || cx.pressed(MouseButton::Middle);
        if pressed {
            let p = cx.pointer().unwrap_or(Pos2::new(f32::NAN, f32::NAN));
            let full = Rect::from_min_size(rect.min, Vec2::new(spec.width, full_h));
            if !full.contains(p) && !spec.holder.contains(p) {
                close(m, id, sub, state);
            }
        }
    }
    crate::window::record_overlay(m, Layer::Menus, Rect::from_min_size(rect.min, Vec2::new(spec.width, full_h)));
    Some(rect)
}

/// Draw the menu chrome (background + outline) with the given corners.
pub(crate) fn chrome(cx: &mut Cx, rect: Rect, corners: MenuCorners) {
    let c = corners.corners(cx.r2());
    cx.rect(rect, c, cx.col(cx.sch.background, 0.0));
    cx.stroke(rect, c, 1.0, cx.col(cx.sch.outline, 0.0), epaint::StrokeKind::Inside);
}
