//! egui ↔ Obsidian key mapping.

use obsidian_ultra_core::input::Key;

pub(crate) fn from_egui(k: egui::Key) -> Option<Key> {
    use egui::Key as E;
    Some(match k {
        E::A => Key::A, E::B => Key::B, E::C => Key::C, E::D => Key::D, E::E => Key::E, E::F => Key::F,
        E::G => Key::G, E::H => Key::H, E::I => Key::I, E::J => Key::J, E::K => Key::K, E::L => Key::L,
        E::M => Key::M, E::N => Key::N, E::O => Key::O, E::P => Key::P, E::Q => Key::Q, E::R => Key::R,
        E::S => Key::S, E::T => Key::T, E::U => Key::U, E::V => Key::V, E::W => Key::W, E::X => Key::X,
        E::Y => Key::Y, E::Z => Key::Z,
        E::Num0 => Key::Zero, E::Num1 => Key::One, E::Num2 => Key::Two, E::Num3 => Key::Three, E::Num4 => Key::Four,
        E::Num5 => Key::Five, E::Num6 => Key::Six, E::Num7 => Key::Seven, E::Num8 => Key::Eight, E::Num9 => Key::Nine,
        E::F1 => Key::F1, E::F2 => Key::F2, E::F3 => Key::F3, E::F4 => Key::F4, E::F5 => Key::F5, E::F6 => Key::F6,
        E::F7 => Key::F7, E::F8 => Key::F8, E::F9 => Key::F9, E::F10 => Key::F10, E::F11 => Key::F11, E::F12 => Key::F12,
        E::F13 => Key::F13, E::F14 => Key::F14, E::F15 => Key::F15, E::F16 => Key::F16, E::F17 => Key::F17, E::F18 => Key::F18,
        E::F19 => Key::F19, E::F20 => Key::F20, E::F21 => Key::F21, E::F22 => Key::F22, E::F23 => Key::F23, E::F24 => Key::F24,
        E::ArrowLeft => Key::Left, E::ArrowRight => Key::Right, E::ArrowUp => Key::Up, E::ArrowDown => Key::Down,
        E::Escape => Key::Escape, E::Enter => Key::Return, E::Space => Key::Space, E::Tab => Key::Tab,
        E::Backspace => Key::Backspace, E::Delete => Key::Delete, E::Home => Key::Home, E::End => Key::End,
        E::PageUp => Key::PageUp, E::PageDown => Key::PageDown, E::Insert => Key::Insert,
        E::Minus => Key::Minus, E::Equals => Key::Equals, E::OpenBracket => Key::LeftBracket, E::CloseBracket => Key::RightBracket,
        E::Backslash => Key::Backslash, E::Semicolon => Key::Semicolon, E::Quote => Key::Quote, E::Comma => Key::Comma,
        E::Period => Key::Period, E::Slash => Key::Slash, E::Backtick => Key::Backquote,
        _ => return None,
    })
}
