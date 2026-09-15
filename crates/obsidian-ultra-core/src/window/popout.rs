//! Pop-out floats for groupboxes and tabboxes.

use epaint::{Pos2, Rect, Vec2};

use crate::frame::{Cx, Layer};
use crate::input::MouseButton;
use crate::node::*;
use crate::ui::Model;

fn pop(m: &mut Model, id: NodeId) -> Option<&mut PopOut> {
    match &mut m.nodes.get_mut(id)?.kind {
        Kind::Groupbox(g) => Some(&mut g.pop),
        Kind::Tabbox(t) => Some(&mut t.pop),
        _ => None,
    }
}
pub(crate) fn set_popped_out(m: &mut Model, id: NodeId, popped: bool, position: Option<Pos2>) {
    let rect = m.nodes[id].rect;
    let Some(p) = pop(m, id) else { return };
    if !p.enabled {
        return;
    }
    if p.popped == popped {
        if popped {
            if let Some(pos) = position {
                p.pos = pos;
            }
        }
        return;
    }
    p.popped = popped;
    p.hold_since = None;
    p.drag_start = None;
    p.did_move = false;
    if popped {
        let width = if rect.width() < 50.0 { 200.0 } else { rect.width() };
        p.width = width;
        p.pos = position.unwrap_or(rect.min);
        let z = m.bump_z();
        if let Some(p) = pop(m, id) {
            p.z = z;
        }
        m.floats.push(id);
        m.raise_float(id);
    } else {
        m.floats.retain(|f| *f != id);
        if m.capture.map(|c| c.node == id && c.sub == cap::POPOUT).unwrap_or(false) {
            m.capture = None;
        }
    }
}

/// Header press-and-hold / drag handling. Called wherever the box header is drawn.
pub(crate) fn handle_header(m: &mut Model, id: NodeId, cx: &mut Cx, header: Rect, box_rect: Rect) {
    let enabled_popped = pop(m, id).map(|p| (p.enabled, p.popped)).unwrap_or((false, false));
    if !enabled_popped.0 {
        return;
    }
    let popped = enabled_popped.1;
    let pointer = cx.pointer();
    let hold_time = m.settings.pop_out_hold_time;
    let threshold = cx.px(m.settings.pop_out_drag_threshold);
    let snap_dist = cx.px(m.settings.pop_out_snap_distance);

    // Begin: press on the header (and not on the collapse chevron / dock buttons handled by callers).
    if m.capture.is_none() && cx.clicked(header) && m.open {
        if let Some(p) = pointer {
            if popped {
                // Only the topmost float receives the drag.
                if super::top_float_at(m, p) != Some(id) {
                    return;
                }
                m.raise_float(id);
            } else if let Some(top) = super::top_float_at(m, p) {
                // A float covers the header.
                if top != id {
                    return;
                }
            }
            let chevron_zone = Rect::from_min_max(Pos2::new(header.max.x - cx.px(34.0), header.min.y), header.max);
            if chevron_zone.contains(p) && matches!(m.nodes[id].kind, Kind::Groupbox(_)) {
                return;
            }
            m.capture = Some(Capture { node: id, sub: cap::POPOUT, start: p, data: [0.0; 4], since: cx.time });
            if let Some(pp) = pop(m, id) {
                pp.hold_since = Some(cx.time);
                pp.press_pos = p;
                pp.drag_start = None;
                pp.did_move = false;
            }
        }
    }

    let Some(c) = m.capture else { return };
    if c.node != id || c.sub != cap::POPOUT {
        return;
    }
    let Some(p) = pointer else { return };
    let down = cx.down(MouseButton::Left);
    let (hold_since, drag_start, did_move, pos) = pop(m, id).map(|pp| (pp.hold_since, pp.drag_start, pp.did_move, pp.pos)).unwrap_or((None, None, false, Pos2::ZERO));
    cx.animate();

    if down {
        // Holding -> dragging after the hold time.
        if drag_start.is_none() {
            if let Some(t0) = hold_since {
                if cx.time - t0 >= hold_time as f64 {
                    if popped {
                        m.raise_float(id);
                        if let Some(pp) = pop(m, id) {
                            pp.drag_start = Some((p, pos));
                        }
                    } else {
                        // Not yet detached: wait for the pointer to travel the threshold.
                        let delta = p - c.start;
                        if delta.length() >= threshold {
                            set_popped_out(m, id, true, Some(box_rect.min));
                            if let Some(pp) = pop(m, id) {
                                pp.drag_start = Some((p, box_rect.min));
                                pp.did_move = true;
                                pp.hold_since = Some(t0);
                            }
                        }
                    }
                }
            }
        } else if let Some((sp, spos)) = drag_start {
            let delta = p - sp;
            if let Some(pp) = pop(m, id) {
                if delta.length() >= threshold {
                    pp.did_move = true;
                }
                pp.pos = spos + delta;
            }
        }
    } else {
        // Release: dock if near the placeholder or dropped inside the main window.
        let was_dragging = drag_start.is_some();
        let now_popped = pop(m, id).map(|pp| pp.popped).unwrap_or(false);
        if was_dragging && now_popped {
            let float_rect = m.nodes[id].rect;
            let center = float_rect.center();
            let placeholder = pop(m, id).map(|pp| pp.placeholder).unwrap_or(Rect::NOTHING);
            let near = m.open && placeholder.is_positive() && (center - placeholder.center()).length() <= snap_dist;
            let inside_main = m.open
                && m.window.map(|w| m.nodes[w].rect.contains(center)).unwrap_or(false)
                && !m.window.map(|w| matches!(&m.nodes[w].kind, Kind::Window(wd) if wd.minimized)).unwrap_or(false);
            if near || (did_move && inside_main) {
                set_popped_out(m, id, false, None);
            }
        }
        if let Some(pp) = pop(m, id) {
            pp.hold_since = None;
            pp.drag_start = None;
            pp.did_move = false;
        }
        m.capture = None;
    }
}

/// Draw a popped-out box as a float.
pub(crate) fn pass_float(m: &mut Model, id: NodeId, cx: &mut Cx) {
    let (pos, width) = match pop(m, id) {
        Some(p) if p.popped => (p.pos, p.width),
        _ => return,
    };
    let visible = m.nodes[id].shown() && m.nodes[id].parent.map(|p| ancestors_shown(m, p)).unwrap_or(true);
    if !visible {
        return;
    }
    // Clamp body height to the screen.
    let screen = m.screen;
    let h = match &m.nodes[id].kind {
        Kind::Groupbox(_) => super::boxes::draw_groupbox(m, id, cx, pos.x, pos.y, width, true),
        Kind::Tabbox(_) => super::boxes::draw_tabbox(m, id, cx, pos.x, pos.y, width, true),
        _ => 0.0,
    };
    let r = Rect::from_min_size(pos, Vec2::new(width, h));
    m.nodes[id].rect = r;
    super::record_overlay(m, Layer::Floats, r.intersect(screen));
}

fn ancestors_shown(m: &Model, mut id: NodeId) -> bool {
    loop {
        let Some(n) = m.nodes.get(id) else { return true };
        if n.destroyed || !n.visible {
            return false;
        }
        match n.parent {
            Some(p) => id = p,
            None => return true,
        }
    }
}
