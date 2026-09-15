//! Custom cursor (crosshair or icon) drawn while the UI is open.

use epaint::{Rect, Vec2};

use crate::frame::{CursorRequest, Cx, Layer};
use crate::ui::Model;

pub(crate) fn pass(m: &mut Model, cx: &mut Cx) {
    if !m.open || !m.settings.show_custom_cursor || m.window.is_none() {
        cx.out.cursor = CursorRequest::Default;
        return;
    }
    let Some(p) = cx.pointer() else {
        cx.out.cursor = CursorRequest::Default;
        return;
    };
    if !m.screen.contains(p) {
        cx.out.cursor = CursorRequest::Default;
        return;
    }
    let screen = m.screen;
    let icon = m.cursor_icon.clone();
    let size = m.cursor_icon_size;
    cx.with_layer(Layer::Cursor, screen, true, |cx| match icon {
        Some(ic) => {
            let r = Rect::from_center_size(p, Vec2::splat(cx.px(size)));
            if !cx.icon(&ic, r, epaint::Color32::WHITE) {
                let mut v = Vec::new();
                crate::draw::crosshair(p, cx.m.s, cx.sch.white, cx.sch.dark, &mut v);
                cx.push_all(v);
            }
        }
        None => {
            let mut v = Vec::new();
            crate::draw::crosshair(p, cx.m.s, cx.sch.white, cx.sch.dark, &mut v);
            cx.push_all(v);
        }
    });
    cx.out.cursor = CursorRequest::Hidden;
    cx.animate();
}
