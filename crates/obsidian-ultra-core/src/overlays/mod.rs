//! Overlays: search, notifications, history, dialogs, floats, loading, unsupported screen, tooltip, cursor.

pub(crate) mod cursor;
pub(crate) mod dialog;
pub(crate) mod floats;
pub(crate) mod history;
pub(crate) mod loading;
pub(crate) mod notify;
pub(crate) mod search;
pub(crate) mod tooltip;
pub(crate) mod unsupported;

use crate::node::*;
use crate::ui::Model;

/// Is `id` inside the loading screen (which stays interactive while the window is closed)?
pub(crate) fn loading_owns(m: &Model, mut id: NodeId) -> bool {
    let Some(l) = m.loading else { return false };
    loop {
        if id == l {
            return true;
        }
        match m.nodes.get(id).and_then(|n| n.parent) {
            Some(p) => id = p,
            None => return false,
        }
    }
}

/// Text of an element for search purposes.
pub(crate) fn element_text(m: &Model, id: NodeId) -> Option<String> {
    Some(match &m.nodes.get(id)?.kind {
        Kind::Label(l) => crate::richtext::strip(&l.text),
        Kind::Button(b) => crate::richtext::strip(&b.text),
        Kind::Toggle(t) => crate::richtext::strip(&t.text),
        Kind::Input(i) => crate::richtext::strip(&i.text),
        Kind::Slider(s) => crate::richtext::strip(&s.text),
        Kind::Dropdown(d) => d.text.as_deref().map(crate::richtext::strip).unwrap_or_default(),
        Kind::Priority(p) => p.text.as_deref().map(crate::richtext::strip).unwrap_or_default(),
        Kind::ProfileCard(p) => {
            let mut s = crate::richtext::strip(&p.title);
            match &p.description {
                crate::types::Description::Text(t) => {
                    s.push(' ');
                    s.push_str(&crate::richtext::strip(t));
                }
                crate::types::Description::Lines(lines) => {
                    for l in lines {
                        if let crate::types::DescriptionLine::Text(t) = l {
                            s.push(' ');
                            s.push_str(&crate::richtext::strip(t));
                        }
                    }
                }
                crate::types::Description::Empty => {}
            }
            s
        }
        _ => return None,
    })
}
